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


if __name__ == "__main__":
    unittest.main()
