#!/usr/bin/env python3
"""Quality and modularity guard for Slint UI components in MusicFrog.

Enforces:
5	1. SLINT-001: Maximum non-comment non-blank lines per hand-written `.slint` file (<= 600 lines).
2. SLINT-002: Modular architecture in `crates/infiltrator-slint/ui/components/`:
   Reusable controls and models must be isolated into cohesive submodules
   (e.g., `theme.slint`, `models.slint`, `base_widgets.slint`, `cards.slint`).
3. SLINT-003: No hardcoded unsemantic raw status colors (e.g. `#0B667F`, `#9A6810`)
   inside page layouts; semantic attributes from `LumenTheme` must be used.

Usage:
    python3 scripts/quality/slint-syntax-guard.py [--mode report|enforce]
    python3 scripts/quality/slint-syntax-guard.py --self-test
"""

from __future__ import annotations

import argparse
import pathlib
import re
import sys

DEFAULT_BUDGET = 600
FORBIDDEN_RAW_COLORS = ("#0B667F", "#9A6810")


def mask_slint_comments(source: str) -> str:
    """Mask single-line and multi-line comments in Slint source."""
    result = []
    i = 0
    n = len(source)
    in_block = False
    while i < n:
        if not in_block and source[i : i + 2] == "//":
            end = source.find("\n", i)
            if end == -1:
                break
            result.append("\n")
            i = end + 1
            continue
        if not in_block and source[i : i + 2] == "/*":
            in_block = True
            i += 2
            continue
        if in_block and source[i : i + 2] == "*/":
            in_block = False
            i += 2
            continue
        if not in_block:
            result.append(source[i])
        i += 1
    return "".join(result)


def non_comment_lines(text: str) -> int:
    masked = mask_slint_comments(text)
    return sum(1 for line in masked.splitlines() if line.strip())


def check_slint_files(repo_root: pathlib.Path, budget: int) -> list[str]:
    violations = []
    slint_dir = repo_root / "crates/infiltrator-slint/ui"
    if not slint_dir.is_dir():
        return violations

    required_components = [
        slint_dir / "components/theme.slint",
        slint_dir / "components/models.slint",
        slint_dir / "components/base_widgets.slint",
        slint_dir / "components/cards.slint",
    ]
    for comp in required_components:
        if not comp.is_file():
            violations.append(f"SLINT-002: Missing required modular component: {comp.relative_to(repo_root)}")

    for path in sorted(slint_dir.rglob("*.slint")):
        text = path.read_text(encoding="utf-8", errors="replace")
        lines = non_comment_lines(text)
        if lines > budget:
            violations.append(
                f"SLINT-001: {path.relative_to(repo_root)} has {lines} code lines, exceeding budget of {budget}"
            )

        if "components/theme.slint" not in str(path):
            for color in FORBIDDEN_RAW_COLORS:
                if re.search(rf"\b{re.escape(color)}\b", text, re.IGNORECASE):
                    violations.append(
                        f"SLINT-003: {path.relative_to(repo_root)} uses hardcoded color '{color}'; use LumenTheme semantic colors instead"
                    )

    return violations


def self_test() -> int:
    sample = """
    // Single line comment
    import { Button } from "std-widgets.slint";
    /* Multi
       line */
    export component Test {
        width: 100px;
    }
    """
    assert non_comment_lines(sample) == 4, f"Expected 4 code lines, got {non_comment_lines(sample)}"
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=["report", "enforce"], default="enforce")
    parser.add_argument("--budget", type=int, default=DEFAULT_BUDGET)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()

    if args.self_test:
        return self_test()

    repo_root = pathlib.Path(__file__).resolve().parents[2]
    issues = check_slint_files(repo_root, args.budget)

    if not issues:
        print("Slint UI quality and modularity guard: 100% PASS")
        return 0

    print(f"Slint UI quality guard: {len(issues)} violation(s) found:", file=sys.stderr)
    for issue in issues:
        print(f"  * {issue}", file=sys.stderr)

    return 1 if args.mode == "enforce" else 0


if __name__ == "__main__":
    sys.exit(main())
