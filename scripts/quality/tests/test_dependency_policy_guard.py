"""Regression checks for the dependency policy guard."""

import importlib.util
from pathlib import Path
import sys
import unittest


QUALITY = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "dependency_policy_guard",
    QUALITY / "dependency-policy-guard.py",
)
GUARD = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = GUARD
SPEC.loader.exec_module(GUARD)


class DependencyPolicyGuardTests(unittest.TestCase):
    def test_repository_policy_is_consistent_with_the_disclosure(self):
        self.assertEqual(GUARD.check(GUARD.REPO_ROOT), [])

    def test_policy_file_is_present_at_the_repository_root(self):
        self.assertTrue((GUARD.REPO_ROOT / "deny.toml").is_file())

    def test_ecosystem_gate_runs_the_policy_from_a_tag_gated_job(self):
        workflow = (GUARD.REPO_ROOT / GUARD.ECOSYSTEM_WORKFLOW).read_text(encoding="utf-8")

        job = GUARD._workflow_job_containing(workflow, "cargo deny check")

        self.assertIsNotNone(job)
        self.assertIn(GUARD.TAG_JOB_CONDITION, job)
        for check_name in ("advisories", "bans", "licenses", "sources"):
            self.assertIn(check_name, job)


if __name__ == "__main__":
    unittest.main()
