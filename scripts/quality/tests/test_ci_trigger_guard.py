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
        'on:\n  push:\n    tags:\n      - "v*"\njobs:\n'
        '  validate-release-ref:\n'
        '    steps:\n'
        '      - run: python3 scripts/quality/android-product-identity-guard.py --release-tag "$GITHUB_REF_NAME"\n'
        '  test-gate:\n'
        '    needs: [validate-release-ref]\n'
        '    uses: ./.github/workflows/test.yml\n'
        '  build-desktop:\n'
        '    needs: [test-gate]\n'
        '  build-android:\n'
        '    needs: [test-gate]\n', encoding="utf-8"
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

    def test_release_test_gate_must_wait_for_exact_tag_validation(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            write_workflows(root)
            release = root / ".github/workflows/release.yml"
            release.write_text(
                release.read_text(encoding="utf-8").replace(
                    "needs: [validate-release-ref]", "needs: [some-other-job]"
                ),
                encoding="utf-8",
            )
            problems = GUARD.source_problems(root)
        self.assertTrue(any("test-gate must wait" in problem for problem in problems))

    def test_each_job_must_be_tag_gated(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            write_workflows(root)
            android = root / ".github/workflows/android.yml"
            android.write_text(
                android.read_text(encoding="utf-8").replace(
                    "  run:\n    if: github.ref_type == 'tag' && startsWith(github.ref_name, 'v')\n",
                    "  dispatch-check:\n"
                    "    if: github.ref_type == 'tag' && startsWith(github.ref_name, 'v')\n"
                    "  android-build:\n"
                    "    runs-on: ubuntu-latest\n",
                ),
                encoding="utf-8",
            )
            problems = GUARD.source_problems(root)
        self.assertTrue(
            any("android.yml job 'android-build'" in problem for problem in problems)
        )


def neutral_workflow_text() -> str:
    """A minimal workflow that satisfies the neutral release policy."""
    return (
        "on:\n"
        "  push:\n"
        "    tags:\n"
        '      - "v*"\n'
        "  workflow_dispatch:\n"
        "jobs:\n"
        "  validate-topology:\n"
        f"{GUARD.NEUTRAL_RELEASE_JOB_CONDITION}\n"
        "    steps:\n"
        "      - run: python3 scripts/quality/publish-order-guard.py\n"
        "  publish:\n"
        f"{GUARD.NEUTRAL_RELEASE_PUBLISH_CONDITION}\n"
        "    env:\n"
        "      CARGO_REGISTRY_TOKEN: ${{ secrets.CARGO_REGISTRY_TOKEN }}\n"
        "    steps:\n"
        "      - name: Require the crates.io API token\n"
        "        shell: bash\n"
        "        run: |\n"
        '          if [ -z "${CARGO_REGISTRY_TOKEN:-}" ]; then\n'
        "            echo \"missing token\"\n"
        "            exit 1\n"
        "          fi\n"
    )


class NeutralReleaseWorkflowRules(unittest.TestCase):
    def test_neutral_release_workflow_is_registered(self):
        self.assertEqual(
            GUARD.EXPECTED_EVENTS[GUARD.NEUTRAL_RELEASE_WORKFLOW],
            {"push", "workflow_dispatch"},
        )

    def test_neutral_release_workflow_declares_the_expected_events(self):
        self.assertEqual(
            GUARD.top_level_events(neutral_workflow_text()),
            {"push", "workflow_dispatch"},
        )

    def test_compliant_neutral_release_workflow_is_accepted(self):
        self.assertEqual(GUARD.neutral_release_problems(neutral_workflow_text()), [])

    def test_branch_push_trigger_is_rejected(self):
        text = neutral_workflow_text().replace(
            '    tags:\n      - "v*"', "    branches: [main]\n"
        )
        problems = GUARD.neutral_release_problems(text)
        self.assertTrue(any("push trigger must match v* tags only" in p for p in problems))

    def test_ungated_job_is_rejected(self):
        text = neutral_workflow_text().replace(
            f"{GUARD.NEUTRAL_RELEASE_JOB_CONDITION}\n", ""
        )
        problems = GUARD.neutral_release_problems(text)
        self.assertTrue(any("must skip non-v* refs" in p for p in problems))

    def test_publish_job_must_be_gated_on_a_tag_push(self):
        text = neutral_workflow_text().replace(
            GUARD.NEUTRAL_RELEASE_PUBLISH_CONDITION,
            GUARD.NEUTRAL_RELEASE_JOB_CONDITION,
        )
        problems = GUARD.neutral_release_problems(text)
        self.assertTrue(any("must gate its publish job on a v* tag push" in p for p in problems))

    def test_publish_job_must_read_the_token_secret(self):
        text = neutral_workflow_text().replace(
            "      CARGO_REGISTRY_TOKEN: ${{ secrets.CARGO_REGISTRY_TOKEN }}\n", ""
        )
        problems = GUARD.neutral_release_problems(text)
        self.assertTrue(any("secrets.CARGO_REGISTRY_TOKEN" in p for p in problems))

    def test_publish_job_must_fail_loudly_without_the_token(self):
        text = neutral_workflow_text().replace("            exit 1\n", "")
        problems = GUARD.neutral_release_problems(text)
        self.assertTrue(any("must fail loudly" in p for p in problems))

    def test_raw_cargo_publish_is_rejected(self):
        text = neutral_workflow_text().replace(
            "      - run: python3 scripts/quality/publish-order-guard.py",
            "      - run: cargo publish -p lurker-core-contract",
        )
        problems = GUARD.neutral_release_problems(text)
        self.assertTrue(any("not through a raw cargo publish" in p for p in problems))

    def test_command_line_token_is_rejected(self):
        text = neutral_workflow_text().replace(
            "      - run: python3 scripts/quality/publish-order-guard.py",
            "      - run: cargo publish --token $TOKEN",
        )
        problems = GUARD.neutral_release_problems(text)
        self.assertTrue(any("must not pass a crates.io token" in p for p in problems))


if __name__ == "__main__":
    unittest.main()
