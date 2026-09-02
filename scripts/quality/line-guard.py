#!/usr/bin/env python3
"""Source line-budget guard for MusicFrog Rust business code.

Rule: one `.rs` business file carries at most `--budget` (default 800)
non-comment, non-blank lines. Comment lines (`//`, including `///` and
`//!`) and block comments (`/* ... */`) are free, as are blank lines —
a file that explains itself is never penalized for its comments.

Scanned roots are the Rust source trees of the workspace (`crates/*/src`).
Dedicated test trees (`tests/` directories) are
excluded: test size is governed by review, not by this budget, and test
layout is enforced by `test-layout-guard.py`.

Usage:
    python3 scripts/quality/line-guard.py [--mode report|enforce] [--budget N]
    python3 scripts/quality/line-guard.py --self-test

Exit status is 0 when no violations are found (or in report mode), 1 when
violations exist in enforce mode.
"""

from __future__ import annotations

import argparse
import pathlib
import sys

DEFAULT_BUDGET = 800

SCAN_ROOTS = ("crates",)


def non_comment_lines(text: str) -> int:
    """Count non-comment, non-blank lines with a small block-comment scanner.

    Line comments run from `//` to end of line; block comments span from
    `/*` to the matching `*/` and nest is NOT supported (matching rustc's
    actual behavior would need nesting — kept simple because the codebase
    does not use nested block comments; both forms inside string literals
    are counted as code, the accepted false-positive cost of a scanner that
    does not tokenize Rust).
    """
    count = 0
    in_block = False
    for raw in text.splitlines():
        stripped = raw.strip()
        if in_block:
            if "*/" in stripped:
                in_block = False
            continue
        if not stripped:
            continue
        if stripped.startswith("//"):
            continue
        if stripped.startswith("/*"):
            if "*/" in stripped:
                # `/* c */ code` — the trailing code after the closed block
                # still counts. A trailing `/*` opened after code on the same
                # line is not tracked (no tokenizing); the codebase does not
                # use that shape.
                if stripped.split("*/", 1)[1].strip():
                    count += 1
            else:
                in_block = True
            continue
        count += 1
    return count


def rs_business_files(repo_root: pathlib.Path) -> list[pathlib.Path]:
    """Every `.rs` business file under the scan roots.

    Excluded: dedicated test trees (`tests/` directories) and test modules
    mounted into `src/` by the repo's `#[path]` convention (filenames
    `*_test.rs` / `*_tests.rs`) — those are test code, not business code.
    """
    files: list[pathlib.Path] = []
    for root in SCAN_ROOTS:
        base = repo_root / root
        if not base.is_dir():
            continue
        for path in base.rglob("*.rs"):
            if "target" in path.parts or "tests" in path.parts:
                continue
            if path.stem.endswith(("_test", "_tests")):
                continue
            files.append(path)
    return sorted(files)


def violations(repo_root: pathlib.Path, budget: int) -> list[tuple[int, pathlib.Path]]:
    """(count, path) for every business file over the budget, worst first."""
    found = [
        (non_comment_lines(path.read_text(encoding="utf-8", errors="replace")), path)
        for path in rs_business_files(repo_root)
    ]
    over = [(count, path) for count, path in found if count > budget]
    return sorted(over, reverse=True)


def self_test() -> int:
    """Assertions for the counting rule; must stay in sync with rustc's
    line-comment and block-comment forms as used in this codebase."""
    assert non_comment_lines("") == 0
    assert non_comment_lines("\n\n   \n") == 0
    assert non_comment_lines("let a = 1;") == 1
    assert non_comment_lines("// header\nlet a = 1;\n/// doc\nlet b = 2;") == 2
    assert non_comment_lines("//! module doc\nlet a = 1;") == 1
    assert non_comment_lines("/* block\n still block */\nlet a = 1;") == 1
    assert non_comment_lines("/* one-line */ let a = 1;") == 1
    assert non_comment_lines("/* whole line is a comment */") == 0
    # Documented limitation: a `/*` opened after code on the same line is
    # not tracked, so its continuation lines count as code (over-counts,
    # i.e. fails safe for a size guard).
    assert non_comment_lines("let a = 1; /* trailing open\nstill block */") == 2
    assert non_comment_lines("let url = \"https://not-a-comment\";") == 1
    assert non_comment_lines("let s = \"// not a comment\";") == 1
    print("self-test OK")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=("report", "enforce"), default="report")
    parser.add_argument("--budget", type=int, default=DEFAULT_BUDGET)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()

    if args.self_test:
        return self_test()

    repo_root = pathlib.Path(__file__).resolve().parents[2]
    over = violations(repo_root, args.budget)
    total = len(rs_business_files(repo_root))
    print(f"line budget guard: budget={args.budget} scanned={total} violations={len(over)}")
    for count, path in over:
        print(f"  {count:>6}  {path.relative_to(repo_root)}")
    if args.mode == "enforce" and over:
        print("enforce mode: split oversized files along business seams", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
