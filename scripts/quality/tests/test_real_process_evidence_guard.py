"""Regression tests for the real-binary evidence guard of the engine drivers.

The guard keeps PUB-006.8 honest: the named evidence target of every driver
stays reachable, the environment contract stays typed and the local entry
points keep printing a machine-readable summary.
"""

import importlib.util
from pathlib import Path
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[3]
QUALITY = ROOT / "scripts/quality"
SPEC = importlib.util.spec_from_file_location(
    "real_process_evidence_guard",
    QUALITY / "real-process-evidence-guard.py",
)
GUARD = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(GUARD)


class RealProcessEvidenceGuardTest(unittest.TestCase):
    """The guard accepts the healthy tree and rejects every rot it names."""

    def setUp(self) -> None:
        self.directory = tempfile.TemporaryDirectory()
        self.root = Path(self.directory.name)
        GUARD._synthetic_tree(self.root)

    def tearDown(self) -> None:
        self.directory.cleanup()

    def test_healthy_tree_passes(self) -> None:
        self.assertEqual(GUARD.check(self.root), [])

    def test_repository_tree_passes(self) -> None:
        self.assertEqual(GUARD.check(ROOT), [])

    def test_unignored_evidence_target_is_rejected(self) -> None:
        target = self.root / "crates/mihomo-driver/tests/real_process_evidence.rs"
        target.write_text(
            target.read_text(encoding="utf-8").replace(
                '#[ignore = "requires the real binary"]', "// ignored"
            ),
            encoding="utf-8",
        )
        problems = GUARD.check(self.root)
        self.assertTrue(any("`#[ignore]`d" in problem for problem in problems), problems)

    def test_untyped_environment_contract_is_rejected(self) -> None:
        contract = (
            self.root
            / "crates/singbox-driver/tests/real_process_evidence/contract.rs"
        )
        contract.write_text(
            contract.read_text(encoding="utf-8").replace(
                "pub enum RealProcessEnvironment {", "pub struct RealProcessEnvironment {"
            ),
            encoding="utf-8",
        )
        problems = GUARD.check(self.root)
        self.assertTrue(any("must be a typed enum" in problem for problem in problems), problems)

    def test_missing_absent_outcome_is_rejected(self) -> None:
        contract = (
            self.root
            / "crates/singbox-driver/tests/real_process_evidence/contract.rs"
        )
        contract.write_text(
            contract.read_text(encoding="utf-8").replace(
                "    Absent { missing: Vec<&'static str> },\n", ""
            ),
            encoding="utf-8",
        )
        problems = GUARD.check(self.root)
        self.assertTrue(any("`Absent` outcome" in problem for problem in problems), problems)

    def test_missing_proof_is_rejected(self) -> None:
        receipt = (
            self.root / "crates/mihomo-driver/tests/real_process_evidence/receipt.rs"
        )
        receipt.write_text(
            receipt.read_text(encoding="utf-8").replace("    NodeSpeedTest,\n", ""),
            encoding="utf-8",
        )
        problems = GUARD.check(self.root)
        self.assertTrue(any("`NodeSpeedTest`" in problem for problem in problems), problems)

    def test_unimplemented_proof_is_rejected(self) -> None:
        proofs = (
            self.root / "crates/singbox-driver/tests/real_process_evidence/proofs.rs"
        )
        proofs.write_text(
            proofs.read_text(encoding="utf-8").replace(
                "    let _ = Proof::TrafficDispatch;\n", ""
            ),
            encoding="utf-8",
        )
        problems = GUARD.check(self.root)
        self.assertTrue(any("traffic_dispatch" in problem for problem in problems), problems)

    def test_unreachable_entry_point_is_rejected(self) -> None:
        smoke = self.root / "scripts/core-backend-smoke-mihomo.sh"
        smoke.write_text(
            smoke.read_text(encoding="utf-8").replace(
                "--test real_process_evidence", "--test other"
            ),
            encoding="utf-8",
        )
        problems = GUARD.check(self.root)
        self.assertTrue(
            any("must run the named evidence target" in problem for problem in problems),
            problems,
        )

    def test_missing_summary_is_rejected(self) -> None:
        smoke = self.root / "scripts/core-backend-smoke-singbox.sh"
        smoke.write_text(
            smoke.read_text(encoding="utf-8").replace(GUARD.SUMMARY_MARKER, "SUMMARY"),
            encoding="utf-8",
        )
        problems = GUARD.check(self.root)
        self.assertTrue(any("summary" in problem for problem in problems), problems)

    def test_hardcoded_environment_key_is_rejected(self) -> None:
        smoke = self.root / "scripts/core-backend-smoke-mihomo.sh"
        smoke.write_text(
            smoke.read_text(encoding="utf-8") + "\nMIHOMO_REAL_CONFIG=/tmp/config\n",
            encoding="utf-8",
        )
        problems = GUARD.check(self.root)
        self.assertTrue(
            any("must not be hardcoded" in problem for problem in problems), problems
        )

    def test_self_test_passes(self) -> None:
        self.assertEqual(GUARD.self_test(), 0)


if __name__ == "__main__":
    unittest.main()
