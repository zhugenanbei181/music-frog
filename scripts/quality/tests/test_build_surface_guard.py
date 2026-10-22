"""Regression checks for the build surface guard."""

import importlib.util
import json
from pathlib import Path
import sys
import unittest


QUALITY = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "build_surface_guard",
    QUALITY / "build-surface-guard.py",
)
GUARD = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = GUARD
SPEC.loader.exec_module(GUARD)


class BuildSurfaceGuardTests(unittest.TestCase):
    def test_repository_build_scripts_match_the_ledger(self):
        self.assertEqual(GUARD.check(GUARD.REPO_ROOT), [])

    def test_every_build_script_declares_its_package_and_side_effects(self):
        document = json.loads(
            (GUARD.REPO_ROOT / GUARD.INVENTORY).read_text(encoding="utf-8")
        )

        for row in document["build_scripts"]:
            self.assertTrue((GUARD.REPO_ROOT / row["path"]).is_file())
            self.assertTrue(row["package"])
            self.assertGreaterEqual(len(row["purpose"].strip()), 12)
            self.assertIsInstance(row["spawns_process"], bool)
            self.assertIsInstance(row["writes_files"], bool)

    def test_discovered_scripts_match_the_registered_paths(self):
        discovered = set(GUARD.discover(GUARD.REPO_ROOT))
        document = json.loads(
            (GUARD.REPO_ROOT / GUARD.INVENTORY).read_text(encoding="utf-8")
        )

        self.assertEqual(discovered, {row["path"] for row in document["build_scripts"]})


if __name__ == "__main__":
    unittest.main()
