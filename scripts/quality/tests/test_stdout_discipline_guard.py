"""Regression checks for the stdout discipline guard."""

import importlib.util
import json
from pathlib import Path
import sys
import unittest


QUALITY = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "stdout_discipline_guard",
    QUALITY / "stdout-discipline-guard.py",
)
GUARD = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = GUARD
SPEC.loader.exec_module(GUARD)


class StdoutDisciplineGuardTests(unittest.TestCase):
    def test_repository_print_sites_match_the_ledger(self):
        self.assertEqual(GUARD.check(GUARD.REPO_ROOT), [])

    def test_inner_layers_keep_a_hard_zero(self):
        document = json.loads(
            (GUARD.REPO_ROOT / GUARD.INVENTORY).read_text(encoding="utf-8")
        )

        for crate in document["hard_zero_crates"]:
            self.assertTrue((GUARD.REPO_ROOT / "crates" / crate).is_dir())
            scanned = GUARD.scan(GUARD.REPO_ROOT)
            for path in scanned:
                if GUARD._crate_of(path) == crate:
                    self.fail(f"{path} prints from a hard-zero crate")

    def test_registered_sites_carry_a_category_and_a_rationale(self):
        document = json.loads(
            (GUARD.REPO_ROOT / GUARD.INVENTORY).read_text(encoding="utf-8")
        )

        for row in document["files"]:
            self.assertIn(row["category"], GUARD.CATEGORIES)
            self.assertGreaterEqual(len(row["rationale"].strip()), 12)
            self.assertGreater(row["sites"], 0)


if __name__ == "__main__":
    unittest.main()
