#!/usr/bin/env python3
"""Fail-closed guard for DUAL-03-07 Overview master switch parity."""

from __future__ import annotations

import argparse
import pathlib
import sys


ROOT = pathlib.Path(__file__).resolve().parents[2]


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


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

    require(
        violations,
        "docs/DUAL_SURFACE_PARITY_MASTER_PLAN.md",
        "DUAL-03-07",
        "系统代理与 TUN 模式双主控大卡",
        "SystemToggleSnapshot",
        "parity-ready",
    )
    require(
        violations,
        "docs/CORE_030_REARCHITECTURE.md",
        "DUAL-03-07",
        "SystemToggleApplication",
        "SetSystemProxy",
        "ToggleTun",
    )
    require(
        violations,
        "docs/DUAL_SURFACE_ARCHITECTURE_AUDIT_030.md",
        "DUAL-03-07",
        "OverviewMasterSwitchButton",
        "host-verified",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/system_toggle.rs",
        "SystemToggle",
        "SystemToggleSnapshot",
        "Pending",
        "Unsupported",
    )
    require(
        violations,
        "crates/infiltrator-application/src/system_toggle_application.rs",
        "SystemToggleApplication",
        "pub fn intent",
        "pending_or_unknown_controls_fail_closed",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/projection.rs",
        "system_toggles: infiltrator_contract::system_toggle::SystemToggleSnapshot",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/surface_projection.rs",
        "SystemToggleApplication::from_surface",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/overview_cards.rs",
        "master_switches_scene_with_snapshot",
        "OverviewMasterSwitchButton",
        "Button",
        "OverviewMasterSwitchText",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/overview_restamp.rs",
        "OverviewMasterSwitchButton",
        "on_overview_master_switch_activated",
        "SystemToggleApplication::intent",
        "UiCommand::SetSystemProxy",
        "UiCommand::ToggleTun",
        "projection.system_toggles",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/overview_tests.rs",
        "overview_master_switch_uses_shared_toggle_policy_and_command_sink",
        "UiCommand::SetSystemProxy { enabled: false }",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/state.rs",
        "pub system_toggles",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/overview_master_switches.rs",
        "overview_master_switches",
        "SystemToggleState",
        "on_press_maybe",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/overview_master_switches_tests.rs",
        "unsupported_master_control_is_not_offered_as_an_action",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/overview.rs",
        "overview_master_switches(state, &lang)",
    )
    require(
        violations,
        "scripts/test.sh",
        "overview-master-switch-guard.py --mode enforce",
    )
    require(
        violations,
        "scripts/test-bevy.sh",
        "overview-master-switch-guard.py\" --mode enforce",
    )

    if violations:
        for violation in violations:
            print(f"overview-master-switch-guard: {violation}", file=sys.stderr)
        print(
            f"overview-master-switch-guard: violations={len(violations)}",
            file=sys.stderr,
        )
        return 1 if args.mode == "enforce" else 0
    print("overview-master-switch-guard: DUAL-03-07 markers=complete violations=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
