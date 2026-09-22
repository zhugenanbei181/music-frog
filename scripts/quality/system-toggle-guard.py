#!/usr/bin/env python3
"""Fail-closed guard for DUAL-02-14 system toggle parity."""

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
        "DUAL-02-14",
        "双端系统级开关 UI 表现 100% 对等",
        "SystemToggleSnapshot/SystemToggleApplication",
        "parity-ready",
    )
    require(
        violations,
        "docs/CORE_030_REARCHITECTURE.md",
        "DUAL-02-14",
        "SetSystemProxy`/`ToggleTun",
    )
    require(
        violations,
        "docs/DUAL_SURFACE_ARCHITECTURE_AUDIT_030.md",
        "DUAL-02-14",
        "重复点击",
        "host-verified",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/system_toggle.rs",
        "enum SystemToggle",
        "enum SystemToggleState",
        "Pending",
        "Unsupported",
        "SystemToggleSnapshot",
        "with_pending",
        "compact_label",
    )
    require(
        violations,
        "crates/infiltrator-application/src/system_toggle_application.rs",
        "pub struct SystemToggleApplication",
        "from_surface",
        "pub fn intent",
        "pending_or_unknown_controls_fail_closed",
        "same_policy_maps_both_controls_to_shared_intents",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/state.rs",
        "pub system_toggles: SystemToggleSnapshot",
        "SystemToggleApplication::from_surface",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/sidebar.rs",
        "system_toggles.system_proxy",
        "system_toggles.tun",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update/system_proxy.rs",
        "SystemToggleApplication::intent",
        "SystemToggle::SystemProxy",
        "with_pending(SystemToggle::SystemProxy",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update/core/runtime_config.rs",
        "SystemToggle::Tun",
        "with_pending(SystemToggle::Tun",
        "with_tun_readback",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/state.rs",
        "fn mini_hud_read_model",
        "runtime.system_toggles.system_proxy.clone()",
        "runtime.system_toggles.tun.clone()",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/mini_hud.rs",
        "mini_hud_read_model",
        "model.system_proxy",
        "model.tun",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/view_sidebar_tests.rs",
        "system_toggle_sidebar_uses_shared_pending_policy",
        "Pending { desired: true }",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/app.rs",
        "SidebarToggleProjection",
        "on_sidebar_system_proxy_activated",
        "on_sidebar_tun_activated",
        "sync_sidebar_toggle_visuals",
        "SystemToggleApplication::intent",
        "ButtonDisabled",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/shell_scene.rs",
        "shell_scene_with_toggles",
        "sidebar_scene_with_toggles",
        "sidebar_system_toggles_scene",
        "compact_label",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/route.rs",
        "SidebarToggleProjection",
        "SystemToggleApplication::from_surface",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/pages_matrix_b_tests.rs",
        "test_sidebar_system_toggles_use_shared_projection_and_commands",
        "Pending {",
        "UiCommand::SetSystemProxy",
        "UiCommand::ToggleTun",
        "SurfaceSnapshotUpdated",
    )
    require(
        violations,
        "scripts/test.sh",
        "system-toggle-guard.py --mode enforce",
    )
    require(
        violations,
        "scripts/test-bevy.sh",
        "scripts/quality/system-toggle-guard.py\" --mode enforce",
    )

    if violations:
        for violation in violations:
            print(f"system-toggle-guard: {violation}", file=sys.stderr)
        print(f"system-toggle-guard: violations={len(violations)}", file=sys.stderr)
        return 1 if args.mode == "enforce" else 0
    print("system-toggle-guard: DUAL-02-14 markers=complete violations=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
