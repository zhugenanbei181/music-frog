"""Keep temporary Apple target build suspension explicit in CI workflows."""

import importlib.util
from pathlib import Path
import tempfile
import unittest

QUALITY = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "apple_ci_deferral_guard",
    QUALITY / "apple-ci-deferral-guard.py",
)
GUARD = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(GUARD)


class AppleCiDeferralRules(unittest.TestCase):
    def test_repository_does_not_schedule_apple_target_builds(self):
        self.assertEqual(GUARD.source_problems(), [])

    def test_macos_matrix_runner_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            workflows = root / ".github/workflows"
            workflows.mkdir(parents=True)
            (workflows / "release.yml").write_text(
                "jobs:\n  build:\n    strategy:\n      matrix:\n"
                "        os: macos-15-intel\n",
                encoding="utf-8",
            )
            problems = GUARD.source_problems(root)
        self.assertTrue(any("schedules a macOS runner" in problem for problem in problems))

    def test_darwin_and_ios_targets_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            workflows = root / ".github/workflows"
            workflows.mkdir(parents=True)
            (workflows / "release.yml").write_text(
                "target: aarch64-apple-darwin\n"
                "ios_target: aarch64-apple-ios\n",
                encoding="utf-8",
            )
            problems = GUARD.source_problems(root)
        self.assertEqual(len(problems), 2)
        self.assertTrue(all("schedules an Apple target" in problem for problem in problems))


if __name__ == "__main__":
    unittest.main()
