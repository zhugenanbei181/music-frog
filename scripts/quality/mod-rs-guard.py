#!/usr/bin/env python3
"""Enforce non-mod.rs module layout across the entire repository.

Under Rust 2018+ module resolution rules and this repository's architectural standard,
the legacy Rust 2015 `foo/mod.rs` layout is strictly forbidden.
All modules must be organized using the standard `foo.rs` (+ optional `foo/` directory) pattern.

Usage:
    python3 scripts/quality/mod-rs-guard.py [--mode report|enforce]
    python3 scripts/quality/mod-rs-guard.py --self-test
"""

from __future__ import annotations

import argparse
from pathlib import Path
import sys
import unittest


REPO_ROOT = Path(__file__).resolve().parents[2]
EXCLUDED_PARTS = {
    ".git",
    ".cargo",
    "target",
    "vendor",
    "node_modules",
}


def find_mod_rs_violations(root: Path) -> list[Path]:
    """Scan root for any mod.rs files outside of excluded directories."""
    violations: list[Path] = []
    for path in root.rglob("mod.rs"):
        if any(part in EXCLUDED_PARTS for part in path.parts):
            continue
        violations.append(path.relative_to(root))
    return sorted(violations)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--mode",
        choices=["report", "enforce"],
        default="enforce",
        help="Whether to fail on violations (default: enforce)",
    )
    parser.add_argument(
        "--self-test",
        action="store_true",
        help="Run self-tests and exit",
    )
    args = parser.parse_args()

    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(TestModRsGuard)
        runner = unittest.TextTestRunner(stream=sys.stdout, verbosity=2)
        result = runner.run(suite)
        return 0 if result.wasSuccessful() else 1

    violations = find_mod_rs_violations(REPO_ROOT)
    if not violations:
        print("[mod-rs-guard] OK: no legacy mod.rs files found across the repository")
        return 0

    print(
        f"[mod-rs-guard] ERROR: found {len(violations)} legacy mod.rs file(s) violating the foo.rs + foo/ standard:",
        file=sys.stderr,
    )
    for v in violations:
        print(f"  - {v}", file=sys.stderr)

    if args.mode == "enforce":
        return 1
    return 0


class TestModRsGuard(unittest.TestCase):
    def test_find_mod_rs_violations_detects_file(self) -> None:
        import tempfile

        with tempfile.TemporaryDirectory() as tmpdir:
            tmppath = Path(tmpdir)
            foo_mod = tmppath / "src" / "foo" / "mod.rs"
            foo_mod.parent.mkdir(parents=True)
            foo_mod.write_text("// test", encoding="utf-8")

            violations = find_mod_rs_violations(tmppath)
            self.assertEqual(violations, [Path("src/foo/mod.rs")])

    def test_find_mod_rs_violations_ignores_target_and_vendor(self) -> None:
        import tempfile

        with tempfile.TemporaryDirectory() as tmpdir:
            tmppath = Path(tmpdir)
            target_mod = tmppath / "target" / "debug" / "build" / "foo" / "mod.rs"
            target_mod.parent.mkdir(parents=True)
            target_mod.write_text("// build", encoding="utf-8")

            violations = find_mod_rs_violations(tmppath)
            self.assertEqual(violations, [])


if __name__ == "__main__":
    sys.exit(main())
