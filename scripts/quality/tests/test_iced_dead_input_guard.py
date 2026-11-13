"""Prove the guard's planted cases and live tighten-only inventory."""

import importlib.util
import io
import pathlib
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[3]
GUARD = ROOT / "scripts/quality/iced-dead-input-guard.py"

specification = importlib.util.spec_from_file_location("iced_dead_input_guard", GUARD)
guard = importlib.util.module_from_spec(specification)
specification.loader.exec_module(guard)


class IcedDeadInputGuardTests(unittest.TestCase):
    def test_self_test_covers_every_planted_case(self):
        result = unittest.TextTestRunner(stream=io.StringIO(), verbosity=0).run(
            unittest.defaultTestLoader.loadTestsFromTestCase(guard.GuardSelfTest)
        )
        self.assertTrue(result.wasSuccessful())
        self.assertEqual(result.testsRun, 8)

    def test_live_tree_findings_are_exactly_registered(self):
        new, stale = guard.audit(guard.VIEW, guard.read_inventory())
        self.assertEqual(new, [])
        self.assertEqual(stale, [])
        self.assertEqual(len(guard.read_inventory()), 7)

    def test_inventory_is_tighten_only_at_exact_sites(self):
        sites = guard.read_inventory()
        self.assertEqual(len({guard.entry_key(site) for site in sites}), len(sites))
        for site in sites:
            self.assertGreater(site["line"], 0)
            self.assertTrue(site["form"])
            self.assertTrue(site["kind"])
            self.assertTrue(site["owner"])
            self.assertTrue(site["reason"])


if __name__ == "__main__":
    unittest.main()
