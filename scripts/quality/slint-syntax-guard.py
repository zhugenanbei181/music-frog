#!/usr/bin/env python3
"""Quality and modularity guard for Slint UI components in MusicFrog.

Enforces:
5	1. SLINT-001: Maximum non-comment non-blank lines per hand-written `.slint` file (<= 600 lines).
2. SLINT-002: Modular architecture in `crates/infiltrator-slint/ui/components/`:
   Reusable controls and models must be isolated into cohesive submodules
   (e.g., `theme.slint`, `models.slint`, `base_widgets.slint`, `cards.slint`).
3. SLINT-003: No colour literal anywhere in `ui/**` except in the palette owner
   (`components/theme.slint`). Every colour a surface asks for must be a
   `LumenTheme` role, so one theme decision reaches every consumer. The rule
   used to be a two-element blacklist (`#0B667F`, `#9A6810`), which let 44 other
   ad-hoc hexes through - among them nine icon-tile inks that measured 1.5:1 to
   2.7:1 against their own card in the light skin, because a step chosen for a
   dark background had been painted on a paper card. `KNOWN_COLOUR_DEBT` is the
   only exception and may only shrink.
4. SLINT-004: A run of mono text must ask the mono role's own weight. The
   shipped `JetBrainsMono-Regular` is the only physical mono face, so a
   `font-weight` of 500/600 next to `mono-family` is a weight with no face:
   the toolkit substitutes or emboldens it, which is the synthesised bold the
   design-resource standard forbids. Nineteen call sites did exactly that,
   borrowing a size and a weight from other roles. Hierarchy in mono text is
   carried by size and colour instead.
5. SLINT-008: `font-weight` is a discrete inventory selection, not an
   animatable number. Interpolating 400 -> 600 renders 401..599, weights for
   which no physical face ships, so the toolkit emboldens a regular face and
   the text reads soft. A state change may animate colour; the weight itself
   must switch at the typed role boundary.
6. SLINT-010: the product version is one identity fact. No page may keep a
   second literal default; every consumer reads `ProductIdentity.version`.

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
PALETTE_OWNER = "components/theme.slint"
# Mono runs may only ask the mono role's own weight.
MONO_FAMILY_MARKER = "mono-family"
MONO_WEIGHT = "DesignType.mono-weight"
WEIGHT_WINDOW = 4
ANIMATED_WEIGHT = re.compile(r"\banimate\s+[^;{]*\bfont-weight\b")
VERSION_LITERAL = re.compile(r'product-version\s*:\s*"\d+\.\d+\.\d+"')
PINNED_WRAPPING_ROW = re.compile(
    r"content-layout := VerticalLayout \{[^}]*height:\s*parent\.height;[^}]*wrap:\s*word-wrap;",
    re.S,
)
# Comments are masked before this runs, so a hex inside prose cannot trip it.
HEX_COLOR = re.compile(r"#[0-9A-Fa-f]{6,8}")
# Shadow/ink primitives and Windows chrome that are not semantic roles. Each
# entry is declared debt: it may only be deleted, never added to without a
# stated reason, and a listed literal that is no longer used is reported so the
# list cannot rot into a silent allowlist.
KNOWN_COLOUR_DEBT = {
    "ui/components/profile_sheets.slint": ("#000000",),
    "ui/components/settings_sheets.slint": ("#000000",),
    "ui/components/overview_order.slint": ("#000000",),
    "ui/components/preferences.slint": ("#00000040",),
    "ui/components/app_chrome.slint": ("#FFFFFF", "#E81123"),
    "ui/lumen-app.slint": ("#FFFFFF", "#000000"),
}


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

        rel = str(path.relative_to(repo_root / "crates/infiltrator-slint"))
        if PALETTE_OWNER not in rel:
            found = set(HEX_COLOR.findall(mask_slint_comments(text)))
            allowed = set(KNOWN_COLOUR_DEBT.get(rel, ()))
            for color in sorted(found - allowed):
                violations.append(
                    f"SLINT-003: {path.relative_to(repo_root)} asks for the colour literal '{color}'; "
                    "use a LumenTheme role (only components/theme.slint declares colours)"
                )
            for color in sorted(allowed - found):
                violations.append(
                    f"SLINT-003: {path.relative_to(repo_root)} no longer uses the debt colour "
                    f"'{color}'; drop it from KNOWN_COLOUR_DEBT"
                )

        if "design-type.slint" not in rel:
            code = mask_slint_comments(text).splitlines()
            for index, line in enumerate(code):
                if MONO_FAMILY_MARKER not in line:
                    continue
                for probe in code[index : index + WEIGHT_WINDOW]:
                    match = re.search(r"font-weight: ([^;]+);", probe)
                    if not match:
                        continue
                    if match.group(1).strip() != MONO_WEIGHT:
                        violations.append(
                            f"SLINT-004: {path.relative_to(repo_root)}:{index + 1} asks mono text for "
                            f"'{match.group(1).strip()}', but only JetBrainsMono-Regular ships; "
                            f"use {MONO_WEIGHT} and carry the hierarchy with size or colour"
                        )
                    break

            for index, line in enumerate(code):
                if ANIMATED_WEIGHT.search(line):
                    violations.append(
                        f"SLINT-008: {path.relative_to(repo_root)}:{index + 1} animates font-weight; "
                        "switch the typed weight role instead of interpolating a physical face"
                    )

            if PINNED_WRAPPING_ROW.search("\n".join(code)):
                violations.append(
                    "SLINT-009: "
                    f"{path.relative_to(repo_root)} pins a wrapping text row to parent.height; "
                    "let the row grow from its own preferred height"
                )
            if VERSION_LITERAL.search("\n".join(code)):
                violations.append(
                    "SLINT-010: "
                    f"{path.relative_to(repo_root)} owns a second product version; "
                    "read ProductIdentity.version"
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
    commented = mask_slint_comments("// #123456 is only a comment\nbackground: #0B667F;")
    assert HEX_COLOR.findall(commented) == ["#0B667F"], HEX_COLOR.findall(commented)
    quoted = mask_slint_comments('theme-color-code: "#016888";')
    assert HEX_COLOR.findall(quoted) == ["#016888"], HEX_COLOR.findall(quoted)
    themed = mask_slint_comments("colorize: LumenTheme.tile-azure;")
    assert HEX_COLOR.findall(themed) == [], HEX_COLOR.findall(themed)
    mono_lines = mask_slint_comments(
        "font-family: DesignType.mono-family;\nfont-weight: DesignType.caption-weight;"
    ).splitlines()
    bad = re.search(r"font-weight: ([^;]+);", mono_lines[1])
    assert bad and bad.group(1).strip() != MONO_WEIGHT, bad
    ok_lines = mask_slint_comments(
        "font-family: DesignType.mono-family;\nfont-weight: DesignType.mono-weight;"
    ).splitlines()
    ok = re.search(r"font-weight: ([^;]+);", ok_lines[1])
    assert ok and ok.group(1).strip() == MONO_WEIGHT, ok
    assert ANIMATED_WEIGHT.search("animate color, font-weight { duration: 120ms; }")
    assert not ANIMATED_WEIGHT.search("animate color { duration: 120ms; }")
    assert PINNED_WRAPPING_ROW.search(
        "content-layout := VerticalLayout { height: parent.height; Text { wrap: word-wrap; } }"
    )
    assert not PINNED_WRAPPING_ROW.search(
        "content-layout := VerticalLayout { Text { wrap: word-wrap; } }"
    )
    assert VERSION_LITERAL.search('in property <string> product-version: "0.68.3";')
    assert not VERSION_LITERAL.search('in property <string> product-version: ProductIdentity.version;')
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
