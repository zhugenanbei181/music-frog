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

    def test_the_desktop_matrices_carry_a_shape_and_slint_needs_none(self):
        # Slint's capture selector already IS the contract page shape, so its
        # matrix carries no shape column. The two desktop surfaces open routes
        # and sections, so each names the shape its frames are judged by, and
        # every one of those shapes comes from the exported table.
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "slint.tsv"
            matrix.generate_surface_tsv("slint", output)
            header = output.read_text(encoding="utf-8").splitlines()[0].split("\t")
        self.assertEqual(header[:4], ["name", "page", "skin", "window_size"])
        self.assertNotIn("shape", header)
        self.assertNotIn("traffic_audit", header)
        for surface in ("bevy", "iced"):
            with tempfile.TemporaryDirectory() as directory:
                output = Path(directory) / f"{surface}.tsv"
                matrix.generate_surface_tsv(surface, output)
                rows = list(csv.DictReader(output.open(encoding="utf-8", newline=""),
                                           delimiter="\t"))
            self.assertEqual(rows[0]["shape"] in matrix.exported_shapes(), True, surface)
            self.assertTrue(all(row["shape"] in matrix.exported_shapes() for row in rows),
                            f"{surface}: a shape the contract does not own")

    def test_a_bevy_section_selector_borrows_the_shape_of_the_route_it_mounts(self):
        # `editor`, `mixin` and `filter` open the profiles page's editor card,
        # `about` opens the settings page's attribution dialog, and the four
        # overview card selectors stay on the overview: none of them is a page
        # morphology, so none may borrow a looser budget than its route.
        self.assertEqual(matrix.bevy_shape("editor"), "profiles")
        self.assertEqual(matrix.bevy_shape("mixin"), "profiles")
        self.assertEqual(matrix.bevy_shape("filter"), "profiles")
        self.assertEqual(matrix.bevy_shape("about"), "about")
        self.assertEqual(matrix.bevy_shape("policy-groups"), "overview")
        self.assertEqual(matrix.bevy_shape("app-routing"), "app_routing")
        self.assertEqual(matrix.bevy_shape("proxies"), "proxies")
        with self.assertRaises(ValueError):
            matrix.bevy_shape("no-such-page")

    def test_shape_for_selector_answers_for_every_surface(self):
        # The harness writes a filtered matrix for a surface it did not
        # generate, so one resolver must answer for all three.
        self.assertEqual(matrix.shape_for_selector("bevy", "mixin"), "profiles")
        self.assertEqual(matrix.shape_for_selector("iced", "mixin"), "editor")
        self.assertEqual(matrix.shape_for_selector("iced", "profiles-grid"), "profiles")
        self.assertEqual(matrix.shape_for_selector("slint", "proxies_workbench"),
                         "proxies_workbench")
        with self.assertRaises(ValueError):
            matrix.shape_for_selector("android", "proxies")

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
