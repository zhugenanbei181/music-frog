#!/usr/bin/env python3
"""Line-budget guard for every MusicFrog Rust source and test file.

Rule: one `.rs` file carries at most 800 (or a stricter `--budget`)
non-comment, non-blank lines. Comment lines (`//`, including `///` and
`//!`) and block comments (`/* ... */`) are free, as are blank lines —
a file that explains itself is never penalized for its comments.

Production, integration tests, path-mounted tests and examples share the same
budget. There is no test exemption. Source-text include! concatenation is banned.

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
from rust_syntax import mask_comments

DEFAULT_BUDGET = 800

SCAN_ROOTS = ("crates",)


def non_comment_lines(text: str) -> int:
    """Count code and literal lines; tokenize nested comments without hiding code."""
    return sum(bool(line.strip()) for line in mask_comments(text).splitlines())


def rs_business_files(repo_root: pathlib.Path) -> list[pathlib.Path]:
    """Every checked-in or new Rust file; only build outputs are excluded."""
    files: list[pathlib.Path] = []
    for root in SCAN_ROOTS:
        base = repo_root / root
        if not base.is_dir():
            continue
        for path in base.rglob("*.rs"):
            if "target" in path.parts:
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
    assert non_comment_lines("let a = 1; /* trailing open\nstill block */") == 1
    assert non_comment_lines("/* start\n*/ let a = 1;") == 1
    assert non_comment_lines("/* outer /* inner */\nouter */ let a = 1;") == 1
    assert non_comment_lines('let s = r#"/* data\n// data */"#;') == 2
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
    if not 0 < args.budget <= DEFAULT_BUDGET:
        parser.error(f"budget must be between 1 and {DEFAULT_BUDGET}; raising the repository ceiling is forbidden")

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
