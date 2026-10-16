#!/usr/bin/env python3
"""Line-budget guard for every MusicFrog Android Kotlin source and test file.

Rule: one hand-written `.kt` file carries at most 800 (or a stricter `--budget`)
non-comment, non-blank lines. Comment lines (`//`, doc comments `/** ... */`)
and block comments (`/* ... */`, including nested comments) are free, as are
blank lines — a file that explains itself is never penalized for its comments.

Production, unit tests and instrumentation tests share the same budget.
Build outputs (build/, generated/) and the UniFFI generated binding
(infiltrator_android.kt) are excluded.

Usage:
    python3 scripts/quality/kt-line-guard.py [--mode report|enforce] [--budget N]
    python3 scripts/quality/kt-line-guard.py --self-test

Exit status is 0 when no violations are found (or in report mode), 1 when
violations exist in enforce mode.
"""

from __future__ import annotations

import argparse
import pathlib
import re
import sys

DEFAULT_BUDGET = 800
SCAN_ROOTS = ("android",)


def mask_kt_comments(source: str) -> str:
    """Mask Kotlin line comments, nested block comments, and preserve string literals."""
    chars: list[str] = []
    i = 0
    n = len(source)
    while i < n:
        # String literal (multiline """ ... """)
        if source[i:i+3] == '"""':
            chars.append('"""')
            i += 3
            while i < n:
                if source[i:i+3] == '"""':
                    chars.append('"""')
                    i += 3
                    break
                chars.append(source[i])
                i += 1
            continue

        # Regular string literal (" ... ")
        if source[i] == '"':
            chars.append('"')
            i += 1
            while i < n:
                if source[i] == '\\' and i + 1 < n:
                    chars.append(source[i:i+2])
                    i += 2
                    continue
                if source[i] == '"':
                    chars.append('"')
                    i += 1
                    break
                if source[i] == '\n':
                    chars.append('\n')
                    i += 1
                    break
                chars.append(source[i])
                i += 1
            continue

        # Character literal (' ... ')
        if source[i] == "'":
            chars.append("'")
            i += 1
            while i < n:
                if source[i] == '\\' and i + 1 < n:
                    chars.append(source[i:i+2])
                    i += 2
                    continue
                if source[i] == "'":
                    chars.append("'")
                    i += 1
                    break
                if source[i] == '\n':
                    chars.append('\n')
                    i += 1
                    break
                chars.append(source[i])
                i += 1
            continue

        # Line comment //
        if source[i:i+2] == '//':
            while i < n and source[i] != '\n':
                i += 1
            continue

        # Block comment /* ... */ with nested comment support in Kotlin
        if source[i:i+2] == '/*':
            depth = 1
            i += 2
            while i < n and depth > 0:
                if source[i:i+2] == '/*':
                    depth += 1
                    i += 2
                elif source[i:i+2] == '*/':
                    depth -= 1
                    i += 2
                else:
                    if source[i] == '\n':
                        chars.append('\n')
                    i += 1
            continue

        chars.append(source[i])
        i += 1

    return "".join(chars)


def non_comment_lines(text: str) -> int:
    """Count non-empty lines after comment masking."""
    masked = mask_kt_comments(text)
    return sum(bool(line.strip()) for line in masked.splitlines())


def kt_business_files(repo_root: pathlib.Path) -> list[pathlib.Path]:
    """Every checked-in hand-written Kotlin file."""
    files: list[pathlib.Path] = []
    for root in SCAN_ROOTS:
        base = repo_root / root
        if not base.is_dir():
            continue
        for path in base.rglob("*.kt"):
            # Exclude build outputs and generated bindings
            if "build" in path.parts or "generated" in path.parts:
                continue
            if path.name == "infiltrator_android.kt":
                continue
            files.append(path)
    return sorted(files)


def violations(repo_root: pathlib.Path, budget: int) -> list[tuple[int, pathlib.Path]]:
    """(count, path) for every business file over the budget, worst first."""
    found = [
        (non_comment_lines(path.read_text(encoding="utf-8", errors="replace")), path)
        for path in kt_business_files(repo_root)
    ]
    over = [(count, path) for count, path in found if count > budget]
    return sorted(over, reverse=True)


def self_test() -> int:
    """Assertions for the counting rule in Kotlin."""
    assert non_comment_lines("") == 0
    assert non_comment_lines("\n\n   \n") == 0
    assert non_comment_lines("val a = 1") == 1
    assert non_comment_lines("// header\nval a = 1\n/** doc */\nval b = 2") == 2
    assert non_comment_lines("/* block\n still block */\nval a = 1") == 1
    assert non_comment_lines("/* one-line */ val a = 1") == 1
    assert non_comment_lines("/* whole line is a comment */") == 0
    assert non_comment_lines("val a = 1 /* trailing open\nstill block */") == 1
    assert non_comment_lines("/* start\n*/ val a = 1") == 1
    # Nested block comments (Kotlin specific)
    assert non_comment_lines("/* outer /* inner */\nouter */ val a = 1") == 1
    assert non_comment_lines('val s = """/* data\n// data */"""') == 2
    assert non_comment_lines('val url = "https://not-a-comment"') == 1
    assert non_comment_lines('val s = "// not a comment"') == 1
    print("kt-line-guard self-test OK")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=("report", "enforce"), default="report")
    parser.add_argument("--budget", type=int, default=DEFAULT_BUDGET)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()

    if args.self_test:
        return self_test()

    repo_root = pathlib.Path(__file__).resolve().parent.parent.parent
    files = kt_business_files(repo_root)
    over = violations(repo_root, args.budget)

    if over:
        for count, path in over:
            rel = path.relative_to(repo_root)
            print(f"VIOLATION [{args.mode}]: {rel}: {count} lines exceeds budget of {args.budget}")
        print(f"kotlin line budget guard: budget={args.budget} scanned={len(files)} violations={len(over)}")
        return 1 if args.mode == "enforce" else 0

    print(f"kotlin line budget guard: budget={args.budget} scanned={len(files)} violations=0")
    return 0


if __name__ == "__main__":
    sys.exit(main())
