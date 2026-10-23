#!/usr/bin/env python3
"""Every workspace member inherits its lint configuration from the root.

The root manifest owns `[workspace.lints]` (one rustc deny group and one
clippy deny group). Every member manifest must say `[lints] workspace = true`
and must not carry a private lint table, or a crate silently drifts away from
the repository-wide lint baseline. The guard is offline: it parses the root
manifest with `tomllib` and checks every workspace member textually.
"""

from __future__ import annotations

import argparse
import re
import sys
import tomllib
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
ROOT_MANIFEST = REPO_ROOT / "Cargo.toml"

EXPECTED_RUST_DENIES = {"ellipsis_inclusive_range_patterns", "keyword_idents"}
EXPECTED_CLIPPY_DENIES = {"dbg_macro", "todo", "unimplemented"}


def workspace_members(root: Path) -> list[Path]:
    document = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))
    members = document.get("workspace", {}).get("members", [])
    manifests: list[Path] = []
    for member in members:
        manifest = root / member / "Cargo.toml"
        if manifest.is_file():
            manifests.append(manifest)
    return manifests


def check(root: Path = REPO_ROOT) -> list[str]:
    problems: list[str] = []
    try:
        document = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))
    except (OSError, tomllib.TOMLDecodeError) as error:
        return [f"Cargo.toml: cannot read the workspace manifest: {error}"]

    lints = document.get("workspace", {}).get("lints", {})
    rust_group = lints.get("rust", {})
    clippy_group = lints.get("clippy", {})
    if not rust_group:
        problems.append("Cargo.toml: [workspace.lints.rust] deny group is missing")
    if not clippy_group:
        problems.append("Cargo.toml: [workspace.lints.clippy] deny group is missing")
    for key in sorted(EXPECTED_RUST_DENIES - set(rust_group)):
        problems.append(f"Cargo.toml: [workspace.lints.rust] lost the '{key}' deny")
    for key in sorted(EXPECTED_CLIPPY_DENIES - set(clippy_group)):
        problems.append(f"Cargo.toml: [workspace.lints.clippy] lost the '{key}' deny")

    for manifest in workspace_members(root):
        text = manifest.read_text(encoding="utf-8", errors="replace")
        relative = manifest.relative_to(root)
        if not re.search(r"^\[lints\]\s*$", text, re.MULTILINE):
            problems.append(f"{relative}: no [lints] table; add `[lints]` + `workspace = true`")
        elif not re.search(r"^\[lints\]\s*\n\s*workspace\s*=\s*true\s*$", text, re.MULTILINE):
            problems.append(f"{relative}: [lints] does not inherit (`workspace = true` missing)")
        if re.search(r"^\[lints\.(rust|clippy)", text, re.MULTILINE):
            problems.append(f"{relative}: private lint table found; inherit from the workspace instead")
    return problems


class LintInheritanceSelfTest(unittest.TestCase):
    def _sandbox(self, files: dict[str, str]) -> "tuple[Path, object]":
        import tempfile

        holder = tempfile.TemporaryDirectory()
        root = Path(holder.name)
        for relative, content in files.items():
            path = root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(content, encoding="utf-8")
        return root, holder

    def test_inheriting_workspace_passes(self):
        root, holder = self._sandbox(
            {
                "Cargo.toml": (
                    "[workspace]\nmembers = [\"crates/one\"]\n"
                    "[workspace.lints.rust]\nkeyword_idents = \"deny\"\nellipsis_inclusive_range_patterns = \"deny\"\n"
                    "[workspace.lints.clippy]\ndbg_macro = \"deny\"\ntodo = \"deny\"\nunimplemented = \"deny\"\n"
                ),
                "crates/one/Cargo.toml": "[package]\nname = \"one\"\n\n[lints]\nworkspace = true\n",
            }
        )
        with holder:
            self.assertEqual(check(root), [])

    def test_member_without_lints_is_reported(self):
        root, holder = self._sandbox(
            {
                "Cargo.toml": (
                    "[workspace]\nmembers = [\"crates/one\"]\n"
                    "[workspace.lints.rust]\nkeyword_idents = \"deny\"\nellipsis_inclusive_range_patterns = \"deny\"\n"
                    "[workspace.lints.clippy]\ndbg_macro = \"deny\"\ntodo = \"deny\"\nunimplemented = \"deny\"\n"
                ),
                "crates/one/Cargo.toml": "[package]\nname = \"one\"\n",
            }
        )
        with holder:
            self.assertTrue(any("no [lints] table" in problem for problem in check(root)))

    def test_private_lint_table_is_reported(self):
        root, holder = self._sandbox(
            {
                "Cargo.toml": (
                    "[workspace]\nmembers = [\"crates/one\"]\n"
                    "[workspace.lints.rust]\nkeyword_idents = \"deny\"\nellipsis_inclusive_range_patterns = \"deny\"\n"
                    "[workspace.lints.clippy]\ndbg_macro = \"deny\"\ntodo = \"deny\"\nunimplemented = \"deny\"\n"
                ),
                "crates/one/Cargo.toml": (
                    "[package]\nname = \"one\"\n\n[lints]\nworkspace = true\n\n[lints.clippy]\nprint_literal = \"deny\"\n"
                ),
            }
        )
        with holder:
            self.assertTrue(any("private lint table" in problem for problem in check(root)))

    def test_missing_workspace_deny_is_reported(self):
        root, holder = self._sandbox(
            {
                "Cargo.toml": (
                    "[workspace]\nmembers = [\"crates/one\"]\n"
                    "[workspace.lints.rust]\nkeyword_idents = \"deny\"\n"
                    "[workspace.lints.clippy]\ndbg_macro = \"deny\"\ntodo = \"deny\"\nunimplemented = \"deny\"\n"
                ),
                "crates/one/Cargo.toml": "[package]\nname = \"one\"\n\n[lints]\nworkspace = true\n",
            }
        )
        with holder:
            self.assertTrue(any("lost the 'ellipsis_inclusive_range_patterns' deny" in problem for problem in check(root)))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--mode", choices=["report", "enforce"], default="report")
    arguments = parser.parse_args()

    if arguments.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(LintInheritanceSelfTest)
        result = unittest.TextTestRunner(verbosity=2).run(suite)
        return 0 if result.wasSuccessful() else 1

    problems = check()
    if problems:
        print("\n".join(problems), file=sys.stderr)
        return 1 if arguments.mode == "enforce" else 0
    print("lint inheritance guard: every member inherits the workspace lint baseline")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
