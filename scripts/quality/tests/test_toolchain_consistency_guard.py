"""Regression checks for the toolchain consistency guard."""

import importlib.util
from pathlib import Path
import sys
import tempfile
import unittest


QUALITY = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "toolchain_consistency_guard",
    QUALITY / "toolchain-consistency-guard.py",
)
GUARD = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = GUARD
SPEC.loader.exec_module(GUARD)


class ToolchainConsistencyGuardTests(unittest.TestCase):
    def test_repository_pins_one_release_everywhere(self):
        self.assertEqual(GUARD.check(GUARD.REPO_ROOT), [])

    def test_floating_workflow_reference_is_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "rust-toolchain.toml").write_text(
                '[toolchain]\nchannel = "1.99.0"\n', encoding="utf-8"
            )
            workflow = root / ".github/workflows/build.yml"
            workflow.parent.mkdir(parents=True)
            workflow.write_text(
                "jobs:\n  build:\n    steps:\n      - uses: dtolnay/rust-toolchain@stable\n",
                encoding="utf-8",
            )

            problems = GUARD.check(root)

        self.assertTrue(any("disagrees with the pinned toolchain" in item for item in problems))

    def test_manifest_rust_version_must_match_the_pin(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "rust-toolchain.toml").write_text(
                '[toolchain]\nchannel = "1.99.0"\n', encoding="utf-8"
            )
            crate = root / "crates/demo"
            crate.mkdir(parents=True)
            (crate / "Cargo.toml").write_text(
                '[package]\nname = "demo"\nversion = "0.1.0"\nrust-version = "1.70"\n',
                encoding="utf-8",
            )

            problems = GUARD.check(root)

        self.assertTrue(any("disagrees with the pinned toolchain" in item for item in problems))


if __name__ == "__main__":
    unittest.main()
