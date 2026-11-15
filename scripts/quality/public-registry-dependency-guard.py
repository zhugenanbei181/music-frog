#!/usr/bin/env python3
"""Keep every workspace dependency reachable without private Git credentials.

The shared UI packages (`zhuge-ui-*`) are published to crates.io. A workspace
that pins them back to a private Git repository makes every tag CI, neutral
crate dry-run and docs.rs build fail with `could not read Username`, even though
the code is public on the registry. The dependency policy already denies
unknown Git sources; this guard adds the explicit ban at the declaration site
and pins the one published version line per package.

Usage:
    python3 scripts/quality/public-registry-dependency-guard.py [--mode report|enforce]
    python3 scripts/quality/public-registry-dependency-guard.py --self-test
"""
from __future__ import annotations

import argparse
import pathlib
import re
import sys
import tempfile
import tomllib
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
WORKSPACE = ROOT / "Cargo.toml"
LOCK = ROOT / "Cargo.lock"
EXPECTED = {
    "zhuge-ui-core": "=0.1.19",
    "zhuge-ui-tokens": "=0.1.2",
    "zhuge-ui-kit-iced": "=0.3.0",
    "zhuge-ui-kit-bevy": "=0.4.2",
}
GIT_SOURCE = "source = \"git+"


def parse_workspace(text: str) -> dict:
    return tomllib.loads(text)


def findings(root: pathlib.Path = ROOT) -> list[str]:
    found: list[str] = []
    workspace_path = root / "Cargo.toml"
    lock_path = root / "Cargo.lock"
    try:
        document = parse_workspace(workspace_path.read_text(encoding="utf-8"))
    except (OSError, tomllib.TOMLDecodeError) as error:
        return [f"Cargo.toml: {error}"]
    deps = document.get("workspace", {}).get("dependencies", {})
    for name, expected in EXPECTED.items():
        declaration = deps.get(name)
        if declaration is None:
            found.append(f"Cargo.toml: {name} is not declared in [workspace.dependencies]")
            continue
        if not isinstance(declaration, dict):
            found.append(f"Cargo.toml: {name} must be a table with one registry version")
            continue
        if declaration.get("version") != expected:
            found.append(f"Cargo.toml: {name} must pin {expected!r}, got {declaration.get('version')!r}")
        for forbidden in ("git", "rev", "branch", "tag", "path"):
            if forbidden in declaration:
                found.append(f"Cargo.toml: {name} must not use {forbidden}= (private source)")
    for name, declaration in deps.items():
        if isinstance(declaration, dict) and any(
            key in declaration for key in ("git", "rev", "branch", "tag")
        ):
            found.append(f"Cargo.toml: {name} declares a Git source; publish it or use a registry")
    try:
        lock = lock_path.read_text(encoding="utf-8")
    except OSError as error:
        return found + [f"Cargo.lock: {error}"]
    for name in EXPECTED:
        block = re.search(
            rf'\[\[package\]\]\nname = "{re.escape(name)}"\n(.*?)(?=\n\[\[package\]\]|\Z)',
            lock,
            re.S,
        )
        if block is None:
            found.append(f"Cargo.lock: {name} is missing")
        elif GIT_SOURCE in block.group(1):
            found.append(f"Cargo.lock: {name} still resolves to a private Git source")
        elif "source = \"registry+" not in block.group(1):
            found.append(f"Cargo.lock: {name} is not a crates.io package")
    return found


class SelfTest(unittest.TestCase):
    def test_the_real_workspace_passes(self) -> None:
        self.assertEqual(findings(), [])

    def test_a_private_git_pin_is_refused(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            original = WORKSPACE.read_text(encoding="utf-8")
            (root / "Cargo.toml").write_text(
                original.replace(
                    'zhuge-ui-core = { version = "=0.1.19" }',
                    'zhuge-ui-core = { git = "https://example.invalid/kit.git", version = "0.1" }',
                ),
                encoding="utf-8",
            )
            (root / "Cargo.lock").write_bytes(LOCK.read_bytes())
            self.assertTrue(any("private Git source" in item or "declares a Git source" in item
                                for item in findings(root)))

    def test_a_version_drift_is_refused(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            (root / "Cargo.toml").write_text(
                WORKSPACE.read_text(encoding="utf-8").replace('"=0.1.19"', '"0.1"'),
                encoding="utf-8",
            )
            (root / "Cargo.lock").write_bytes(LOCK.read_bytes())
            self.assertTrue(any("must pin" in item for item in findings(root)))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=["report", "enforce"], default="report")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(SelfTest)
        result = unittest.TextTestRunner(verbosity=1).run(suite)
        return 0 if result.wasSuccessful() else 1
    found = findings()
    for item in found:
        print(f"VIOLATION [{args.mode}]: {item}")
    print(f"public registry dependency guard: total={len(found)}")
    return 1 if args.mode == "enforce" and found else 0


if __name__ == "__main__":
    sys.exit(main())
