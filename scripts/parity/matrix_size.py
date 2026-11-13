"""How many scenarios a full capture matrix declares, derived from the matrix.

Three readers needed this one number and each kept its own copy:

  * `scripts/quality/receipt-freshness-guard.py` typed `FULL_MATRIX_SIZE = 211`
    and treated a run as a publication only when its `status=ok` rows equalled
    it;
  * `scripts/quality/slint-dock-material-guard.py` typed the same 211 again and
    compared it against the `scenario_count` a run declares at start-up;
  * `scripts/parity/space_utilization.py` inferred it as **the largest
    `scenario_count` ever published** by any run directory.

A hand-typed count is a trap the moment the matrix grows, which `SPACEUTIL-021`
now explicitly requires (new viewport tiers and navigation bands need new
frames). With the literal left at 211 while the matrix holds 212 rows, the
freshness guard silently refuses to recognise the newest full batch as a
publication and keeps judging the older corpus, and the dock-material guard
finds no run at all. The third variant is worse in a different direction: an
interrupted run writes its declared count before capturing anything, so one
killed batch against a longer matrix redefines "full" for every later run and
no frame matches it any more.

The canonical matrix is the only place that knows what a full batch is, so it is
the only source for its size. Readers that must reject an under-covered
publication compare against this count.
"""
from __future__ import annotations

import csv
import pathlib
import re
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
CANONICAL_MATRIX = {
    "slint": "scripts/capture_slint_scenarios.tsv",
    "iced": "scripts/capture_iced_scenarios.tsv",
    "bevy": "scripts/capture_bevy_scenarios.tsv",
}


def matrix_scenario_count(surface: str, root: pathlib.Path | None = None) -> int:
    """Non-empty scenario rows the canonical matrix declares for a surface."""
    if surface not in CANONICAL_MATRIX:
        raise KeyError(f"no canonical capture matrix registered for surface {surface!r}")
    base = root or ROOT
    path = base / CANONICAL_MATRIX[surface]
    with path.open(encoding="utf-8", newline="") as handle:
        reader = csv.reader(handle, delimiter="\t")
        rows = list(reader)
    if not rows:
        raise ValueError(f"{path}: an empty matrix cannot define a full batch")
    header = rows[0]
    if not header or not re.search(r"name|scenario", header[0], re.I):
        raise ValueError(f"{path}: first column must name the scenario, found {header[:1]}")
    count = sum(1 for row in rows[1:] if row and row[0].strip())
    if not count:
        # A matrix with no scenario rows would make every reader conclude that
        # zero frames are required, which is how a corpus silently stops being
        # evidence. Refuse to answer rather than answer "0".
        raise ValueError(f"{path}: declares no scenarios")
    return count


class Fixture(unittest.TestCase):
    """The count must come from the file, not from anyone's memory."""

    def setUp(self) -> None:
        self._tmp = __import__("tempfile").TemporaryDirectory()
        self.root = pathlib.Path(self._tmp.name)
        (self.root / "scripts").mkdir()

    def tearDown(self) -> None:
        __import__("shutil").rmtree(self._tmp.name, ignore_errors=True)

    def write(self, body: str) -> None:
        (self.root / CANONICAL_MATRIX["slint"]).write_text(body, encoding="utf-8")

    def test_it_counts_rows_and_ignores_the_header_and_blanks(self) -> None:
        self.write("name\tpage\na\toverview\nb\tlogs\n\t\n\nc\tsettings\n")
        self.assertEqual(matrix_scenario_count("slint", self.root), 3)

    def test_a_longer_matrix_raises_the_number_itself(self) -> None:
        self.write("name\tpage\n" + "".join(f"s{i}\tpage\n" for i in range(212)))
        self.assertEqual(matrix_scenario_count("slint", self.root), 212)

    def test_a_header_only_matrix_is_an_error_not_zero(self) -> None:
        self.write("name\tpage\n")
        with self.assertRaises(ValueError):
            matrix_scenario_count("slint", self.root)

    def test_an_unregistered_surface_is_an_error(self) -> None:
        with self.assertRaises(KeyError):
            matrix_scenario_count("android", self.root)


def self_test() -> int:
    suite = unittest.TestLoader().loadTestsFromTestCase(Fixture)
    result = unittest.TextTestRunner(verbosity=0).run(suite)
    ok = result.testsRun - len(result.failures) - len(result.errors)
    print(f"matrix-size: self-test {ok}/{result.testsRun} passed")
    return 0 if result.wasSuccessful() else 1


if __name__ == "__main__":
    raise SystemExit(self_test())
