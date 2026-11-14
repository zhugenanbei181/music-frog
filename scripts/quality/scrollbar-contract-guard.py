#!/usr/bin/env python3
"""Enforce the shared scrollbar contract across the desktop surfaces.

`infiltrator_contract::scrollbar` owns one canonical scrollbar spec: a slim
floating rail that rests invisible and fades in on interaction. A surface that
ignores it renders a permanently visible track, which is what the reference
layout rejects.

Usage:
    python3 scripts/quality/scrollbar-contract-guard.py [--mode report|enforce]
    python3 scripts/quality/scrollbar-contract-guard.py --self-test
"""

from __future__ import annotations

import argparse
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
CONTRACT = ROOT / "crates/infiltrator-contract/src/scrollbar.rs"
ICED_SCROLLBAR = ROOT / "crates/infiltrator-iced/src/view/scrollbar.rs"
ICED_SRC = ROOT / "crates/infiltrator-iced/src"
BEVY_SRC = ROOT / "crates/infiltrator-bevy-ui/src"
SLINT_UI = ROOT / "crates/infiltrator-slint/ui"

# A widget construction is `scrollable(content)`; `fn scrollable(` is the
# `Operation` trait's callback that *measures* an existing scrollable and must
# not be mistaken for building a second, unstyled one (b268: `locate.rs`
# implements that trait twice and painted no scrollbar at all).
RAW_SCROLLBAR_CALL = re.compile(r"(?<![:\w])scrollable\(")
OPERATION_METHOD = re.compile(r"\bfn\s+scrollable\s*\(")


def iced_scrollbar_source() -> str:
    return ICED_SCROLLBAR.read_text(encoding="utf-8")


def check() -> list[str]:
    violations: list[str] = []

    contract = CONTRACT.read_text(encoding="utf-8")
    for metric in ("track_width_px", "idle_opacity", "active_opacity", "content_gutter_px"):
        if metric not in contract:
            violations.append(f"the canonical scrollbar contract lost '{metric}'")

    iced = iced_scrollbar_source()
    if "ScrollbarMetrics::DEFAULT" not in iced:
        violations.append("the Iced surface does not read the canonical scrollbar metrics")
    if "idle_opacity" not in iced or "active_opacity" not in iced:
        violations.append("the Iced surface ignores the shared auto-hide opacity policy")
    for geometry in ("track_width_px", "thumb_margin_px", "scroller_width", "content_gutter_px"):
        if geometry not in iced:
            violations.append(f"the Iced scrollbar ignores canonical geometry: {geometry}")
    for axis in ("is_vertical_scrollbar_hovered", "is_horizontal_scrollbar_hovered"):
        if axis not in iced:
            violations.append(f"the Iced scrollbar ignores axis-specific hover state: {axis}")
    if re.search(r"matches!\s*\(\s*status\s*,\s*scrollable::Status::Hovered\s*\{\s*\.\.\s*\}", iced):
        violations.append("the Iced scrollbar reveals both rails when content is hovered")

    # Every scrollable in the Iced surface must go through the shared helpers.
    for path in sorted(ICED_SRC.rglob("*.rs")):
        if path.name in ("scrollbar.rs", "capture_geometry.rs"):
            continue
        source = path.read_text(encoding="utf-8")
        for number, line in enumerate(source.splitlines(), 1):
            if (RAW_SCROLLBAR_CALL.search(line)
                    and not OPERATION_METHOD.search(line)):
                violations.append(
                    f"{path.relative_to(ROOT)}:{number} builds a scrollbar outside the shared style"
                )

    return violations


def main() -> int:
    parser = argparse.ArgumentParser(description="Shared scrollbar contract guard")
    parser.add_argument("--mode", choices=["report", "enforce"], default="report")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        assert not check()
        print("scrollbar contract guard self-test: PASS")
        return 0
    violations = check()
    for violation in violations:
        print(f"VIOLATION [{args.mode}]: {violation}")
    print(f"scrollbar contract guard: total={len(violations)}")
    return 1 if args.mode == "enforce" and violations else 0


if __name__ == "__main__":
    sys.exit(main())
