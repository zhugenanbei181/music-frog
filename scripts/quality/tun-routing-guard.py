#!/usr/bin/env python3
"""Fail-closed guard for DUAL-02-03 strict-route traffic capture."""

from __future__ import annotations

import argparse
import pathlib
import sys


ROOT = pathlib.Path(__file__).resolve().parents[2]


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def require(violations: list[str], path: str, *markers: str) -> None:
    text = read(path)
    compact_text = " ".join(text.split())
    for marker in markers:
        compact_marker = " ".join(marker.split())
        rustfmt_marker = compact_marker.replace(" }", ", }")
        if marker not in text and compact_marker not in compact_text and rustfmt_marker not in compact_text:
            violations.append(f"{path} missing {marker!r}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--mode", choices=["report", "enforce"], default="enforce")
    args = parser.parse_args()
    violations: list[str] = []

    require(
        violations,
        "docs/DUAL_SURFACE_PARITY_MASTER_PLAN.md",
        "`DUAL-02-03` 严格路由与全局流量劫持",
        "strict-route",
        "auto-route",
        "`parity-ready`",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/command.rs",
        "SetTunAutoRoute { enabled: bool }",
        "SetTunStrictRoute { enabled: bool }",
        "Self::SetTunAutoRoute { .. }",
        "Self::SetTunStrictRoute { .. }",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/surface_snapshot.rs",
        "pub tun_auto_route: bool",
        "pub tun_strict_route: bool",
    )
    require(
        violations,
        "crates/infiltrator-domain/src/runtime.rs",
        "pub auto_route: bool",
        "pub strict_route: bool",
    )
    require(
        violations,
        "crates/infiltrator-application/src/runtime_query_application.rs",
        "pub async fn set_tun_auto_route",
        "pub async fn set_tun_strict_route",
        "pub async fn set_tun_enabled",
        "current_tun_routing",
        '"strict-route": strict_route',
        "TUN routing readback mismatch",
        "TUN enable readback mismatch",
        "strict_route_enables_auto_route_and_auto_route_off_clears_strict_route",
    )
    require(
        violations,
        "crates/infiltrator-application/src/command_application/dispatch.rs",
        "CommandIntent::SetTunAutoRoute",
        "CommandIntent::SetTunStrictRoute",
        "CommandIntent::ToggleTun",
        "set_tun_auto_route(enabled)",
        "set_tun_strict_route(enabled)",
        "set_tun_enabled(enabled)",
    )
    require(
        violations,
        "crates/infiltrator-application/src/command_application/dispatch.rs",
        "tun_auto_route",
        "tun_strict_route",
    )

    require(
        violations,
        "crates/infiltrator-application/src/surface_reader/page_builders.rs",
        "tun.auto_route",
        "tun.strict_route",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update/core/runtime_config.rs",
        "RuntimeQueryApplication::new(gateway)",
        ".set_tun_enabled(enabled)",
        ".set_tun_auto_route(enabled)",
        ".set_tun_strict_route(enabled)",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/state.rs",
        "settings.tun_auto_route",
        "settings.tun_strict_route",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/iced_six_advancements_wave5_tests.rs",
        "test_shared_surface_route_flags_update_the_iced_projection",
        "tun_auto_route: true",
        "tun_strict_route: true",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/command.rs",
        "SetTunAutoRoute(bool)",
        "SetTunStrictRoute(bool)",
        "CommandIntent::SetTunAutoRoute",
        "CommandIntent::SetTunStrictRoute",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/settings_core.rs",
        "TunRouteToggleKind",
        "TunEnableToggle",
        "ValueChange<bool>",
        "checkbox_scene",
        "UiCommand::SetTunAutoRoute",
        "UiCommand::SetTunStrictRoute",
        "UiCommand::ToggleTun",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/settings_tun.rs",
        "tun_route_toggle_scene",
        "tun_enable_toggle_scene",
        "projection.tun_auto_route",
        "projection.tun_strict_route",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/settings.rs",
        "on_tun_route_changed",
        "on_tun_enabled_changed",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/settings_tun.rs",
        "apply_tun_toggle_projection",
        "commands.entity(entity).insert(Checked)",
        "TunRouteToggleKind",
        "TunEnableToggle",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/pages_matrix_b_tests.rs",
        "test_settings_tun_route_checkboxes_submit_shared_commands",
        "UiCommand::SetTunAutoRoute(false)",
        "UiCommand::SetTunStrictRoute(true)",
        "route checkbox should follow the shared projection",
        "test_settings_tun_enable_checkbox_submits_shared_command",
        "UiCommand::ToggleTun { enabled: false }",
    )

    if violations:
        for violation in violations:
            print(f"tun-routing-guard: {violation}", file=sys.stderr)
        print(f"tun-routing-guard: violations={len(violations)}", file=sys.stderr)
        return 1 if args.mode == "enforce" else 0
    print("tun-routing-guard: DUAL-02-03 markers=complete violations=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
