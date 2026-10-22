"""Regression checks for the panic boundary guard."""

import importlib.util
import json
from pathlib import Path
import sys
import unittest


QUALITY = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "panic_boundary_guard",
    QUALITY / "panic-boundary-guard.py",
)
GUARD = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = GUARD
SPEC.loader.exec_module(GUARD)


class PanicBoundaryGuardTests(unittest.TestCase):
    def test_repository_boundaries_match_the_ledger(self):
        self.assertEqual(GUARD.check(GUARD.REPO_ROOT), [])

    def test_every_boundary_names_the_failure_it_contains(self):
        document = json.loads(
            (GUARD.REPO_ROOT / GUARD.INVENTORY).read_text(encoding="utf-8")
        )

        for row in document["boundaries"]:
            self.assertIn(row["category"], GUARD.CATEGORIES)
            self.assertGreaterEqual(len(row["rationale"].strip()), 12)
            self.assertGreater(row["sites"], 0)

    def test_no_workspace_profile_aborts_on_panic(self):
        self.assertEqual(GUARD.aborting_profiles(GUARD.REPO_ROOT), [])


if __name__ == "__main__":
    unittest.main()
