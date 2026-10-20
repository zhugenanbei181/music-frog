from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[3]
SCRIPT = ROOT / "scripts/quality/android-interaction-receipt-guard.py"
SPEC = importlib.util.spec_from_file_location("android_interaction_receipt_guard", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
guard = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(guard)


class AndroidInteractionReceiptGuardTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="android-receipt-guard-test-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        (self.root / "scripts/parity").mkdir(parents=True)
        (self.root / "scripts/parity/feature_evidence.tsv").write_text(
            (ROOT / "scripts/parity/feature_evidence.tsv").read_text(encoding="utf-8"),
            encoding="utf-8",
        )
        (self.root / "evidence").mkdir()
        self.document = guard._synthetic_receipt(self.root, self.root / "evidence")

    def test_the_scenario_table_binds_every_step_to_the_catalogue(self) -> None:
        self.assertEqual(guard.scenario_problems(ROOT), [])

    def test_the_flow_refuses_a_physical_device(self) -> None:
        self.assertEqual(guard.flow_problems(ROOT), [])

    def test_the_self_test_passes(self) -> None:
        self.assertEqual(guard.self_test(), 0)

    def test_a_provable_receipt_set_is_accepted(self) -> None:
        self.assertEqual(guard.receipt_problems(self.root, self.document), [])

    def test_a_receipt_without_its_facts_is_refused(self) -> None:
        with self.subTest("apk digest"):
            document = json.loads(json.dumps(self.document))
            document["receipts"][0]["apk_sha256"] = ""
            self.assertTrue(
                any(
                    "APK digest" in issue
                    for issue in guard.receipt_problems(self.root, document)
                )
            )
        with self.subTest("business id"):
            document = json.loads(json.dumps(self.document))
            document["receipts"][0]["business_id"] = ""
            self.assertTrue(
                any(
                    "business id" in issue
                    for issue in guard.receipt_problems(self.root, document)
                )
            )
        with self.subTest("logcat evidence"):
            document = json.loads(json.dumps(self.document))
            document["receipts"][0]["logcat"] = {}
            self.assertTrue(
                any(
                    "logcat evidence" in issue
                    for issue in guard.receipt_problems(self.root, document)
                )
            )

    def test_a_receipt_set_that_proves_nothing_is_refused(self) -> None:
        document = json.loads(json.dumps(self.document))
        clone = json.loads(json.dumps(document["receipts"][0]))
        clone["surface"] = "bevy"
        clone["step_id"] = "navigation"
        document["receipts"].append(clone)
        self.assertTrue(
            any(
                "identical" in issue
                for issue in guard.receipt_problems(self.root, document)
            )
        )

    def test_an_unresolved_action_cannot_be_reported_as_proven(self) -> None:
        document = json.loads(json.dumps(self.document))
        document["receipts"][0]["actions"] = [
            {"kind": "tap_text", "resolved_by": "unresolved"}
        ]
        self.assertTrue(
            any(
                "unresolved action" in issue
                for issue in guard.receipt_problems(self.root, document)
            )
        )


if __name__ == "__main__":
    unittest.main()
