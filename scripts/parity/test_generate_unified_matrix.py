"""Keep the capture matrix's default skin and four-theme inspection contract explicit."""

import csv
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import generate_unified_matrix as matrix


class UnifiedMatrixTests(unittest.TestCase):
    def test_business_rows_default_to_forest_and_theme_rows_stay_complete(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "bevy.tsv"
            matrix.generate_surface_tsv("bevy", output)
            with output.open(encoding="utf-8", newline="") as stream:
                rows = list(csv.DictReader(stream, delimiter="\t"))

        self.assertEqual(len(rows), 182)
        theme_names = {
            f"{page}-{theme}"
            for page, *_ in matrix.CORE_PAGES
            for theme in matrix.THEMES
        }
        business = [row for row in rows if row["name"] not in theme_names]
        inspection = [row for row in rows if row["name"] in theme_names]

        self.assertEqual(len(business), 118)
        self.assertEqual({row["skin"] for row in business}, {matrix.BUSINESS_SKIN})
        self.assertEqual(len(inspection), 64)
        for theme in matrix.THEMES:
            self.assertEqual(
                sum(row["skin"] == theme for row in inspection),
                len(matrix.CORE_PAGES),
            )
        self.assertEqual(len({row["name"] for row in rows}), len(rows))

    def test_every_iced_shape_is_a_page_shape_the_contract_exports(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "iced.tsv"
            matrix.generate_surface_tsv("iced", output)
            with output.open(encoding="utf-8", newline="") as stream:
                rows = list(csv.DictReader(stream, delimiter="\t"))

        self.assertEqual(len(rows), 182)
        self.assertIn("shape", rows[0])
        exported = matrix.exported_shapes()
        self.assertTrue(exported)
        for row in rows:
            self.assertIn(row["shape"], exported, row["name"])
        business = [row for row in rows if row["interaction"]]
        self.assertEqual(len(business), 118)
        self.assertEqual(
            {row["interaction"] for row in business},
            {feature[0] for feature in matrix.parse_features()},
        )

    def test_only_the_iced_matrix_carries_a_shape_column(self):
        for surface in ("slint", "bevy"):
            with tempfile.TemporaryDirectory() as directory:
                output = Path(directory) / f"{surface}.tsv"
                matrix.generate_surface_tsv(surface, output)
                header = output.read_text(encoding="utf-8").splitlines()[0].split("\t")
            self.assertEqual(header, ["name", "page", "skin", "window_size"])
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "iced.tsv"
            matrix.generate_surface_tsv("iced", output)
            header = output.read_text(encoding="utf-8").splitlines()[0].split("\t")
        self.assertEqual(
            header,
            ["name", "page", "shape", "skin", "window_size", "interaction", "sidebar_expansion"],
        )

    def test_a_surface_owned_row_keeps_its_place_and_its_columns(self):
        """Twelve of the Slint rows sit inside the generated block, not after it."""
        fresh = [
            "name\tpage\tskin\twindow_size",
            "first\tpage\tforest\t1180x780",
            "second\tpage\tforest\t720x480",
        ]
        committed = [
            "name\tpage\tskin\twindow_size\tscroll\tsidebar_expansion\tinteraction",
            "first\tpage\tforest\t1180x780\t\tcollapsed\tfirst-scenario",
            "hand\tpage\tforest\t720x480\t\tcollapsed\thand-scenario",
            "second\tpage\tforest\t720x480\t\tcollapsed\tsecond-scenario",
        ]
        merged, kept = matrix.merge_rows(fresh, committed)

        self.assertEqual(merged, committed)
        self.assertEqual(kept, 1)

    def test_a_generated_page_change_keeps_the_surface_columns(self):
        fresh = ["name\tpage\tskin\twindow_size", "row\tnew_page\tforest\t1180x780"]
        committed = [
            "name\tpage\tskin\twindow_size\tinteraction",
            "row\told_page\tforest\t1180x780\tsome-interaction",
        ]
        merged, _ = matrix.merge_rows(fresh, committed)

        self.assertEqual(merged[1], "row\tnew_page\tforest\t1180x780\tsome-interaction")

    def test_a_matrix_that_lost_a_generated_row_is_reported(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "capture_slint_scenarios.tsv"
            fresh = matrix.generated_rows("slint")
            path.write_text("\n".join([fresh[0], *fresh[2:]]) + "\n", encoding="utf-8")
            original = matrix.ROOT
            matrix.ROOT = Path(directory)
            try:
                findings = matrix.matrix_findings("slint")
            finally:
                matrix.ROOT = original

        self.assertTrue(findings, "a generated row went missing without a finding")
        self.assertIn("missing", findings[0])

    def test_the_committed_matrices_match_the_generator(self):
        """Regenerating the two matrices this batch owns is a no-op."""
        for surface in ("slint", "iced"):
            path = matrix.ROOT / f"scripts/capture_{surface}_scenarios.tsv"
            committed = path.read_text(encoding="utf-8").splitlines()
            merged, _ = matrix.merge_rows(matrix.generated_rows(surface), committed)
            self.assertEqual(merged, committed, surface)
            self.assertEqual(matrix.matrix_findings(surface), [], surface)


if __name__ == "__main__":
    unittest.main()
