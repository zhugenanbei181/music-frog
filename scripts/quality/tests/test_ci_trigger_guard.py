"""Regression tests for the tag-only expensive CI policy."""

import importlib.util
from pathlib import Path
import tempfile
import unittest

QUALITY = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "ci_trigger_guard",
    QUALITY / "ci-trigger-guard.py",
)
GUARD = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(GUARD)


def write_workflows(root: Path, *, android_events: str = "  workflow_dispatch:\n") -> None:
    workflows = root / ".github/workflows"
    workflows.mkdir(parents=True)
    (workflows / "release.yml").write_text(
        'on:\n  push:\n    tags:\n      - "v*"\njobs:\n  build:\n', encoding="utf-8"
    )
    (workflows / "test.yml").write_text(
        "on:\n  workflow_call:\njobs:\n  test:\n"
        "    if: github.ref_type == 'tag' && startsWith(github.ref_name, 'v')\n",
        encoding="utf-8",
    )
    for name in ("android-device.yml", "android.yml", "bevy.yml", "parity.yml"):
        (workflows / name).write_text(
            f"on:\n{android_events}jobs:\n  run:\n"
            "    if: github.ref_type == 'tag' && startsWith(github.ref_name, 'v')\n",
            encoding="utf-8",
        )


class CiTriggerRules(unittest.TestCase):
    def test_repository_workflows_are_tag_gated(self):
        self.assertEqual(GUARD.source_problems(), [])

    def test_non_tag_automatic_trigger_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            write_workflows(root, android_events="  push:\n    branches: [main]\n")
            problems = GUARD.source_problems(root)
        self.assertTrue(any("android.yml events" in problem for problem in problems))

    def test_release_branch_trigger_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            write_workflows(root)
            release = root / ".github/workflows/release.yml"
            release.write_text(
                release.read_text(encoding="utf-8").replace(
                    '    tags:\n      - "v*"', '    branches: [main]\n'
                ),
                encoding="utf-8",
            )
            problems = GUARD.source_problems(root)
        self.assertTrue(any("release.yml push trigger" in problem for problem in problems))


if __name__ == "__main__":
    unittest.main()
