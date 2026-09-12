#!/usr/bin/env python3
"""Fail-closed guard for dual-surface responsive (multi-size) parity.

This guard is deliberately NUMERIC, not a string-presence check: the main
regression it defends against is the two breakpoint systems silently drifting
apart. It verifies that:

1. `infiltrator-contract` holds the authoritative thresholds 600/840/1200.
2. `infiltrator-bevy-widgets` mirrors the SAME three numbers (it cannot depend
   on contract, so the mirror is enforced here).
3. Iced actually consumes window resize events and drives the shared viewport
   projection (a dead contract is the failure mode this catches).
4. Bevy's fluid grid system stays registered.
5. The authoritative ledger documents the item and both surfaces' tests exist.

Usage:
    python3 scripts/quality/responsive-parity-guard.py [--mode report|enforce]
"""

from __future__ import annotations

import argparse
import pathlib
import re
import sys


ROOT = pathlib.Path(__file__).resolve().parents[2]

CONTRACT = "crates/infiltrator-contract/src/responsive_viewport.rs"
BEVY_THEME = "crates/infiltrator-bevy-widgets/src/theme.rs"
ICED_MSG = "crates/infiltrator-iced/src/types/message.rs"
ICED_SUB = "crates/infiltrator-iced/src/subscription.rs"
ICED_UPDATE = "crates/infiltrator-iced/src/update.rs"
ICED_SIDEBAR = "crates/infiltrator-iced/src/view/sidebar.rs"
ICED_ROOT = "crates/infiltrator-iced/src/view_root.rs"
ICED_OVERVIEW = "crates/infiltrator-iced/src/view/overview.rs"
ICED_PROXIES = "crates/infiltrator-iced/src/view/proxies.rs"
ICED_DRAWER = "crates/infiltrator-iced/src/view_root/connection_drawer.rs"
BEVY_WIDGETS_LIB = "crates/infiltrator-bevy-widgets/src/lib.rs"
BEVY_OVERVIEW = "crates/infiltrator-bevy-ui/src/pages/overview.rs"
BEVY_PROXIES = "crates/infiltrator-bevy-ui/src/pages/proxies.rs"
LEDGER = "docs/RESPONSIVE_PARITY_LEDGER.md"

EXPECTED = {
    "COMPACT_MAX_PX": 600.0,
    "MEDIUM_MAX_PX": 840.0,
    "EXPANDED_MAX_PX": 1200.0,
}


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def extract_const(text: str, name: str) -> float | None:
    match = re.search(
        rf"pub const {name}\s*:\s*f32\s*=\s*([0-9]+(?:\.[0-9]+)?)\s*;", text
    )
    return float(match.group(1)) if match else None


def require(violations: list[str], path: str, *markers: str) -> None:
    text = read(path)
    for marker in markers:
        if marker not in text:
            violations.append(f"{path} missing {marker!r}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--mode", choices=["report", "enforce"], default="enforce")
    args = parser.parse_args()
    violations: list[str] = []

    contract_text = read(CONTRACT)
    bevy_text = read(BEVY_THEME)

    # 1 + 2. Numeric threshold agreement between the two breakpoint systems.
    for name, expected in EXPECTED.items():
        contract_value = extract_const(contract_text, name)
        bevy_value = extract_const(bevy_text, name)
        if contract_value != expected:
            violations.append(
                f"{CONTRACT} {name}={contract_value!r}, expected {expected}"
            )
        if bevy_value != contract_value:
            violations.append(
                f"{BEVY_THEME} {name}={bevy_value!r} must mirror contract {contract_value!r}"
            )

    # 3. Iced must actually consume resize and drive the shared viewport.
    require(violations, ICED_MSG, "WindowResized")
    require(violations, ICED_SUB, "resize_events", "WindowResized")
    require(violations, ICED_UPDATE, "WindowResized", "ResponsiveViewportSnapshot")
    require(violations, ICED_SIDEBAR, "sidebar_for_tier")
    require(violations, ICED_ROOT, "sidebar_for_tier")

    # 3b. Both surfaces must derive grids from the shared tier operators, not
    # hardcoded column counts.
    require(violations, ICED_OVERVIEW, "metrics_grid_columns")
    require(violations, ICED_PROXIES, "proxy_grid_columns")
    require(violations, ICED_ROOT, "content_padding_px")
    require(violations, BEVY_OVERVIEW, "sync_overview_metrics_columns")
    require(violations, BEVY_PROXIES, "sync_proxies_node_columns")

    # 3c. Paginated lists and side panels scale with the window tier.
    require(violations, ICED_UPDATE, "list_page_rows")
    require(violations, ICED_DRAWER, "detail_panel_width_px")

    # 4. Bevy fluid grid engine stays registered.
    require(violations, BEVY_WIDGETS_LIB, "sync_fluid_grid_layout")

    # 5. Ledger + dual-surface tests exist.
    require(
        violations,
        LEDGER,
        "双端多尺寸弹性",
        "DUAL-15-01",
        "DUAL-03-14",
        "唯一权威断点模型",
    )
    iced_tests = ROOT / "crates/infiltrator-iced/tests/gui/responsive_elasticity_tests.rs"
    bevy_tests = (
        ROOT / "crates/infiltrator-bevy-ui/tests/headless/responsive_ui_tests.rs"
    )
    if not iced_tests.is_file():
        violations.append(f"missing Iced responsive test: {iced_tests}")
    if not bevy_tests.is_file():
        violations.append(f"missing Bevy responsive test: {bevy_tests}")
    else:
        require(
            violations,
            "crates/infiltrator-bevy-ui/tests/headless/responsive_ui_tests.rs",
            "test_bevy_breakpoint_mirrors_shared_contract_at_boundaries",
        )

    if violations:
        for violation in violations:
            print(f"responsive-parity-guard: {violation}", file=sys.stderr)
        print(
            f"responsive-parity-guard: violations={len(violations)}", file=sys.stderr
        )
        return 1 if args.mode == "enforce" else 0
    print("responsive-parity-guard: breakpoints=600/840/1200 dual-surface consumers=ok violations=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
