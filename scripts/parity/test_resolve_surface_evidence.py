"""Negative evidence-closure tests: deleting, renaming or misattributing an anchor fails."""
import copy
import unittest
from resolve_surface_evidence import discovered_tests, validate


class ClosureTests(unittest.TestCase):
    def setUp(self):
        self.specs = [{"id": "confirm", "required_paths": ["success", "cancel"]}]
        self.manifest = [dict(feature_id="confirm", surface=s, status="pending", reason="pixels pending",
                              standard_scenario="confirm-standard", compact_scenario="confirm-compact")
                         for s in ("iced", "bevy")]
        self.evidence = [dict(feature_id="confirm", surface=s, level=level, status="anchored",
                              test_id=f"{s}::tests::cancel", paths="success,cancel", reason="-")
                         for s in ("iced", "bevy") for level in ("contract", "scenario")]
        self.discovery = {f"{s}::tests::cancel": f"infiltrator-{s if s == 'iced' else 'bevy-ui'}" for s in ("iced", "bevy")}

    def report(self, **overrides):
        args = dict(manifest=self.manifest, evidence=self.evidence, catalogue=self.specs, discovery=self.discovery)
        args.update(overrides)
        return validate(**args)

    def test_exact_discovery_is_not_complete_parity(self):
        report = self.report()
        self.assertEqual(report["status"], "pass")
        self.assertEqual(report["pending"], [["confirm", "bevy"], ["confirm", "iced"]])
        self.assertFalse(report["complete"])
        self.assertEqual(self.report(require_complete=True)["status"], "fail")

    def test_deleted_or_renamed_test_fails_closed(self):
        discovery = {"iced::tests::renamed": "infiltrator-iced", "bevy::tests::cancel": "infiltrator-bevy-ui"}
        report = self.report(discovery=discovery)
        self.assertEqual(len(report["dangling"]), 2)
        self.assertEqual(report["status"], "fail")

    def test_other_package_with_same_test_name_cannot_satisfy_anchor(self):
        self.discovery["bevy::tests::cancel"] = "infiltrator-iced"
        self.assertEqual(self.report()["status"], "fail")

    def test_duplicate_or_missing_peer_cell_is_rejected(self):
        self.assertEqual(self.report(manifest=self.manifest + [self.manifest[0]])["status"], "fail")
        self.assertEqual(self.report(manifest=self.manifest[:1])["status"], "fail")
        self.assertEqual(self.report(evidence=self.evidence[:3])["status"], "fail")

    def test_unknown_feature_and_incomplete_cancel_path_are_rejected(self):
        self.assertEqual(self.report(catalogue=[{"id": "other", "required_paths": ["success"]}])["status"], "fail")
        self.evidence[1]["paths"] = "success"
        self.assertEqual(self.report()["status"], "fail")

    def test_pending_cannot_hide_an_anchor_or_become_ready(self):
        self.evidence[1].update(status="pending", test_id="-", paths="-", reason="missing cancellation")
        self.manifest[0]["status"] = "ready"
        report = self.report()
        self.assertEqual(report["status"], "fail")
        self.assertTrue(any("scenario evidence" in error for error in report["invalid"]))
        self.assertTrue(any("pixels" in error for error in report["invalid"]))
        self.evidence[1]["test_id"] = "iced::tests::cancel"
        self.assertTrue(any("cannot carry an anchor" in error for error in self.report()["invalid"]))

    def test_ready_requires_both_verified_viewports(self):
        for row in self.manifest:
            row["status"] = "ready"
        receipts = {( "confirm", s, v) for s in ("iced", "bevy") for v in ("standard", "compact")}
        self.assertTrue(self.report(receipts=receipts)["complete"])
        receipts.remove(("confirm", "bevy", "compact"))
        self.assertEqual(self.report(receipts=receipts)["status"], "fail")

    def test_discovery_uses_binary_package_and_excludes_ignored_tests(self):
        payload = {"rust-suites": {"iced": {"package-name": "infiltrator-iced",
                   "testcases": {"tests::cancel": {"ignored": False}, "tests::ignored": {"ignored": True}}}}}
        self.assertEqual(discovered_tests(payload), {"iced::tests::cancel": "infiltrator-iced"})
        with self.assertRaises(ValueError):
            discovered_tests({"rust-suites": {}})


if __name__ == "__main__":
    unittest.main()
