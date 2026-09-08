#!/usr/bin/env python3
"""Fail-closed guard for DUAL-02-09 Mihomo IPv6 routing policy."""

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
        "`DUAL-02-09` IPv6 内核流量与 TUN 转发策略开关",
        "Ipv6RoutingSnapshot",
        "PATCH `ipv6`",
        "默认 `true`",
        "sysctl",
        "`parity-ready`",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/ipv6.rs",
        "Ipv6RoutingSnapshot",
        "pub enabled: bool",
        "pub tun_enabled: bool",
        "pub revision: u64",
        "does not claim to mutate",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/command.rs",
        "SetIpv6Routing { enabled: bool }",
        "Self::SetIpv6Routing { .. }",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/capability.rs",
        "Ipv6Routing",
    )
    require(
        violations,
        "crates/infiltrator-domain/src/runtime.rs",
        "pub ipv6: bool",
        "default_ipv6_enabled",
    )
    require(
        violations,
        "crates/mihomo-api/src/types.rs",
        "pub ipv6: bool",
        "default_ipv6_enabled",
        "ipv6: value.ipv6",
    )
    require(
        violations,
        "crates/mihomo-api/src/types_test.rs",
        "assert!(!config.ipv6)",
        "missing ipv6 follows Mihomo's documented true default",
    )
    require(
        violations,
        "crates/infiltrator-application/src/runtime_query_ipv6.rs",
        "pub async fn set_ipv6_routing",
        '"ipv6": enabled',
        "IPv6 routing readback mismatch",
        "Ipv6RoutingSnapshot::new",
    )
    require(
        violations,
        "crates/infiltrator-application/src/runtime_query_ipv6.rs",
        "ipv6_routing_patch_reads_back_core_policy_and_tun_context",
        "ipv6_routing_fails_closed_when_controller_ignores_patch",
    )
    require(
        violations,
        "crates/infiltrator-application/src/command_application.rs",
        "CommandIntent::SetIpv6Routing",
        ".set_ipv6_routing(enabled)",
    )
    require(
        violations,
        "crates/infiltrator-application/src/core_application.rs",
        '"set_ipv6_routing"',
    )
    require(
        violations,
        "crates/infiltrator-application/src/surface_reader.rs",
        "ipv6_routing: config.map_or_else",
        "value.ipv6",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/surface_snapshot.rs",
        "pub ipv6_routing: crate::ipv6::Ipv6RoutingSnapshot",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/types/runtime.rs",
        "pub ipv6_enabled: bool",
        "ipv6_enabled: bool",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/app.rs",
        "ipv6_routing: Default::default()",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/state.rs",
        "pub ipv6_routing:",
        "self.runtime.ipv6_routing = settings.ipv6_routing",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update/core/runtime_config.rs",
        "Message::SetIpv6Routing(enabled)",
        ".set_ipv6_routing(enabled)",
        "ipv6_enabled: self.runtime.ipv6_routing.enabled",
        "config.ipv6",
        "config.ipv6_enabled",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update.rs",
        "Message::SetIpv6Routing(_)",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/types/message.rs",
        "SetIpv6Routing(bool)",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/types/message/debug.rs",
        "Message::SetIpv6Routing(enabled)",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/settings.rs",
        "settings_ipv6_routing",
        "Message::SetIpv6Routing",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/app_state_tests.rs",
        "ipv6_enabled: false",
        "state.runtime.ipv6_routing.enabled",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/iced_six_advancements_wave5_tests.rs",
        "Ipv6RoutingSnapshot::new(2, false, true)",
        "state.runtime.ipv6_routing.tun_enabled",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/command.rs",
        "SetIpv6Routing { enabled: bool }",
        "CommandIntent::SetIpv6Routing",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/settings_core.rs",
        "Ipv6Routing,",
        "pub ipv6_routing:",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/settings_ipv6.rs",
        "Ipv6RoutingToggle",
        "on_changed",
        "UiCommand::SetIpv6Routing",
        "apply_projection",
        "format_status",
        "防止旁路泄漏",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/settings_tun.rs",
        "settings_ipv6::scene",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/settings.rs",
        "settings_ipv6::on_changed",
        "settings_ipv6::apply_projection",
        "SettingsLineKind::Ipv6Routing",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/surface_projection.rs",
        "ipv6_routing: value.ipv6_routing",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/pages_matrix_b_tests.rs",
        "test_settings_ipv6_routing_projects_and_submits_live_intent",
        "Ipv6RoutingToggle",
        "UiCommand::SetIpv6Routing { enabled: true }",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/surface.rs",
        "Capability::Ipv6Routing",
    )
    require(
        violations,
        "crates/infiltrator-android/src/runtime.rs",
        "Capability::Ipv6Routing",
        "supports(Capability::Ipv6Routing)",
    )
    require(
        violations,
        "crates/infiltrator-android/src/composition.rs",
        "CommandApplication",
        ".with_runtime(gateway)",
    )
    require(
        violations,
        "crates/infiltrator-ios/src/lib.rs",
        "Capability::Ipv6Routing",
        "iOS controller gateway is not exposed by the native host",
        "!capabilities.supports(Capability::Ipv6Routing)",
    )

    if violations:
        for violation in violations:
            print(f"ipv6-routing-guard: {violation}", file=sys.stderr)
        print(f"ipv6-routing-guard: violations={len(violations)}", file=sys.stderr)
        return 1 if args.mode == "enforce" else 0
    print("ipv6-routing-guard: DUAL-02-09 markers=complete violations=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
