"""Unit tests for the docs.rs preview and public-API documentation guard.

The guard owns three contracts: every neutral crate declares the docs.rs
build configuration, every neutral crate publishes at least one doctest, and
the docs.rs-equivalent `cargo doc --all-features --no-deps` run is free of
rustdoc warnings. The negative tests prove that a missing metadata section, a
crate without a doctest and a rustdoc warning are all rejected.
"""

from __future__ import annotations

import importlib.util
from pathlib import Path
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[3]
SCRIPT = ROOT / "scripts/quality/rustdoc-guard.py"
SPEC = importlib.util.spec_from_file_location("rustdoc_guard", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
GUARD = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(GUARD)

# A cargo output that carries exactly one rustdoc warning.
WARNING_OUTPUT = (
    "   Documenting lurker-core-contract v0.50.0\n"
    "warning: missing documentation for a struct\n"
    "  --> crates/lurker-core-contract/src/lib.rs:7:1\n"
    "   |\n"
    "7  | pub struct Demo;\n"
    "   | ^^^^^^^^^^^^^^\n"
    "   |\n"
    "   = note: `#[warn(missing_docs)]` on by default\n"
    "\n"
    "warning: `lurker-core-contract` (lib doc) generated 1 warning\n"
    "    Finished `dev` profile [unoptimized + debuginfo] target(s)\n"
)

# A cargo output that only carries a cargo-level notice.
CLEAN_OUTPUT = (
    "   Documenting lurker-core-contract v0.50.0\n"
    "    Finished `dev` profile [unoptimized + debuginfo] target(s)\n"
)


def write_neutral_tree(root: Path, *, metadata: bool = True, doctest: bool = True) -> Path:
    """Build a synthetic tree that mirrors the seven neutral crates."""
    for crate in GUARD.NEUTRAL_CRATES:
        crate_root = root / "crates" / crate
        (crate_root / "src").mkdir(parents=True)
        manifest = [f'[package]', f'name = "{crate}"', 'version = "0.50.0"']
        if metadata:
            manifest += [
                "[package.metadata.docs.rs]",
                "all-features = true",
                'rustdoc-args = ["--cfg", "docsrs"]',
                'targets = ["x86_64-unknown-linux-gnu"]',
            ]
        (crate_root / "Cargo.toml").write_text("\n".join(manifest) + "\n", encoding="utf-8")
        body = "//! Demo crate.\n"
        if doctest:
            body += "//!\n//! ```\n//! let x = 1;\n//! ```\n"
        (crate_root / "src" / "lib.rs").write_text(body, encoding="utf-8")
    return root


class RustdocMetadataTests(unittest.TestCase):
    def test_repository_neutral_crates_declare_the_docs_rs_metadata(self) -> None:
        self.assertEqual(GUARD.metadata_problems(ROOT), [])

    def test_every_neutral_crate_declares_the_same_metadata_shape(self) -> None:
        for crate in GUARD.NEUTRAL_CRATES:
            import tomllib

            document = tomllib.loads(
                GUARD.crate_manifest(ROOT, crate).read_text(encoding="utf-8")
            )
            metadata = GUARD.docs_rs_metadata(document)
            self.assertIsNotNone(metadata, crate)
            assert metadata is not None
            self.assertEqual(metadata.get("all-features"), True, crate)
            self.assertIn("--cfg", metadata.get("rustdoc-args", []), crate)
            self.assertIn("docsrs", metadata.get("rustdoc-args", []), crate)
            self.assertTrue(metadata.get("targets"), crate)

    def test_guard_rejects_a_manifest_without_the_docs_rs_metadata(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = write_neutral_tree(Path(directory), metadata=False)

            problems = GUARD.metadata_problems(root)

            self.assertTrue(
                any("missing `[package.metadata.docs.rs]`" in problem for problem in problems),
                problems,
            )

    def test_guard_rejects_metadata_without_the_docsrs_rustdoc_arg(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = write_neutral_tree(Path(directory))
            manifest = root / "crates" / "lurker-core-contract" / "Cargo.toml"
            manifest.write_text(
                '[package]\nname = "lurker-core-contract"\nversion = "0.50.0"\n'
                "[package.metadata.docs.rs]\n"
                "all-features = true\n"
                'rustdoc-args = ["--cfg", "not_docsrs"]\n'
                'targets = ["x86_64-unknown-linux-gnu"]\n',
                encoding="utf-8",
            )

            problems = GUARD.metadata_problems(root)

            self.assertTrue(
                any("--cfg docsrs" in problem for problem in problems), problems
            )

    def test_guard_rejects_metadata_without_any_target(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = write_neutral_tree(Path(directory))
            manifest = root / "crates" / "lurker-core-contract" / "Cargo.toml"
            manifest.write_text(
                '[package]\nname = "lurker-core-contract"\nversion = "0.50.0"\n'
                "[package.metadata.docs.rs]\n"
                "all-features = true\n"
                'rustdoc-args = ["--cfg", "docsrs"]\n',
                encoding="utf-8",
            )

            problems = GUARD.metadata_problems(root)

            self.assertTrue(
                any("`targets` list" in problem for problem in problems), problems
            )


class RustdocDoctestTests(unittest.TestCase):
    def test_every_neutral_crate_publishes_at_least_one_doctest(self) -> None:
        self.assertEqual(GUARD.doctest_problems(ROOT), [])
        for crate in GUARD.NEUTRAL_CRATES:
            self.assertGreaterEqual(GUARD.doctest_count(ROOT, crate), 1, crate)

    def test_guard_rejects_a_crate_without_any_doctest(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = write_neutral_tree(Path(directory), doctest=False)

            problems = GUARD.doctest_problems(root)

            self.assertTrue(
                any("publishes no doctest" in problem for problem in problems), problems
            )

    def test_a_fenced_block_outside_a_doc_comment_is_not_a_doctest(self) -> None:
        self.assertEqual(GUARD.doc_code_blocks("```\nlet x = 1;\n```\n"), [])
        self.assertEqual(len(GUARD.doc_code_blocks("/// ```\n/// let x = 1;\n/// ```\n")), 1)


class RustdocWarningTests(unittest.TestCase):
    def test_guard_rejects_a_rustdoc_warning(self) -> None:
        def fake_runner(_root, _crate, _timeout):
            return 0, WARNING_OUTPUT

        with tempfile.TemporaryDirectory() as directory:
            root = write_neutral_tree(Path(directory))

            problems = GUARD.docs_problems(root, runner=fake_runner)

            self.assertTrue(
                any("rustdoc reported 1 warning(s)" in problem for problem in problems),
                problems,
            )

    def test_guard_rejects_a_failed_cargo_doc_run(self) -> None:
        def fake_runner(_root, _crate, _timeout):
            return 101, WARNING_OUTPUT

        with tempfile.TemporaryDirectory() as directory:
            root = write_neutral_tree(Path(directory))

            problems = GUARD.docs_problems(root, runner=fake_runner)

            self.assertTrue(
                any("failed (exit code 101)" in problem for problem in problems), problems
            )

    def test_guard_accepts_a_clean_cargo_doc_run(self) -> None:
        def fake_runner(_root, _crate, _timeout):
            return 0, CLEAN_OUTPUT

        with tempfile.TemporaryDirectory() as directory:
            root = write_neutral_tree(Path(directory))

            self.assertEqual(GUARD.docs_problems(root, runner=fake_runner), [])

    def test_a_cargo_notice_without_a_source_location_is_not_a_rustdoc_warning(self) -> None:
        self.assertEqual(
            GUARD.rustdoc_warning_lines("warning: unused manifest key: package.demo\n"),
            [],
        )
        self.assertEqual(
            GUARD.rustdoc_warning_lines(
                "warning: missing documentation for a struct\n"
                "  --> crates/demo/src/lib.rs:3:1\n"
            ),
            ["warning: missing documentation for a struct"],
        )


class RustdocSelfTestTests(unittest.TestCase):
    def test_guard_self_test_passes(self) -> None:
        self.assertEqual(GUARD.self_test(), 0)


if __name__ == "__main__":
    unittest.main()
