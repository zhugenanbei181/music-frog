"""Regression checks for the unsafe traceability guard."""

import importlib.util
import json
from pathlib import Path
import sys
import unittest


QUALITY = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "unsafe_traceability_guard",
    QUALITY / "unsafe-traceability-guard.py",
)
GUARD = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = GUARD
SPEC.loader.exec_module(GUARD)


class UnsafeTraceabilityGuardTests(unittest.TestCase):
    def test_repository_ledger_matches_the_source(self):
        self.assertEqual(GUARD.check(GUARD.REPO_ROOT), [])

    def test_every_registered_file_carries_a_category_and_a_rationale(self):
        document = json.loads(
            (GUARD.REPO_ROOT / GUARD.INVENTORY).read_text(encoding="utf-8")
        )

        for row in document["unsafe_files"]:
            self.assertIn(row["category"], GUARD.CATEGORIES)
            self.assertGreaterEqual(len(row["rationale"].strip()), 12)
            self.assertGreater(row["operations"], 0)
            self.assertLessEqual(row["max_undocumented"], row["operations"])

    def test_vendored_reference_sources_stay_outside_the_scan(self):
        document = json.loads(
            (GUARD.REPO_ROOT / GUARD.INVENTORY).read_text(encoding="utf-8")
        )

        for row in document["unsafe_files"]:
            self.assertFalse(row["path"].startswith("crates/infiltrator-slint/lumen-source/"))


if __name__ == "__main__":
    unittest.main()
