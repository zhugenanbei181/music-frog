#!/usr/bin/env python3
"""Fail-closed guard for DUAL-02-12 physical-link roaming recovery."""

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
        "DUAL-02-12",
        "物理网卡漫游与默认网关感知",
        "NetworkObservation/NetworkRoamingSnapshot",
        "auto-route",
        "参数化路由命令",
        "typed unsupported",
        "parity-ready",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/network_roaming.rs",
        "NetworkInterfaceKind",
        "NetworkInterfaceSnapshot",
        "NetworkObservation",
        "NetworkRoamingRepairRequest",
        "NetworkRoamingEvent",
        "RoutesRepaired",
        "NetworkRoamingStatus",
        "NetworkRoamingSnapshot",
        "pub fn unsupported",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/command.rs",
        "RefreshNetworkRoaming",
        "RepairNetworkRoutes",
        "Self::RefreshNetworkRoaming",
        "Self::RepairNetworkRoutes",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/capability.rs",
        "NetworkRoaming",
    )
    require(
        violations,
        "crates/infiltrator-domain/src/network_roaming.rs",
        "select_active_interface",
        "route_repair_required",
        "gateway_migration_requests_repair_when_tun_auto_route_is_live",
        "no_physical_route_is_not_an_empty_successful_repair_plan",
    )
    require(
        violations,
        "crates/infiltrator-ports/src/network_roaming.rs",
        "trait NetworkRoamingPort",
        "async fn observe",
        "async fn repair",
        "NetworkRoamingRepairRequest",
    )
    require(
        violations,
        "crates/infiltrator-application/src/network_roaming_application.rs",
        "pub struct NetworkRoamingApplication",
        "pub async fn refresh",
        "pub async fn refresh_from",
        "pub async fn force_repair",
        "NetworkRoamingPort",
        "select_active_interface",
        "migration_repairs_routes_and_publishes_a_shared_snapshot",
        "migration_is_projected_without_host_mutation_when_auto_route_is_off",
        "force_repair_requires_a_composed_live_gateway",
    )
    require(
        violations,
        "crates/infiltrator-application/src/command_application.rs",
        "with_network_roaming",
    )

    require(
        violations,
        "crates/infiltrator-application/src/command_application/dispatch.rs",
        "CommandIntent::RefreshNetworkRoaming",
        "CommandIntent::RepairNetworkRoutes",
        "self.network_roaming()?.refresh()",
        "self.network_roaming()?.force_repair()",
    )
    require(
        violations,
        "crates/infiltrator-application/src/surface_reader.rs",
        "with_network_roaming",
        "read_network_roaming",
        "network_roaming,",
    )
    require(
        violations,
        "crates/infiltrator-application/src/surface_reader_services.rs",
        "application.refresh().await",
        "Capability::NetworkRoaming",
        "read_network_roaming",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/surface_snapshot.rs",
        "pub network_roaming: crate::network_roaming::NetworkRoamingSnapshot",
    )
    require(
        violations,
        "crates/infiltrator-application/src/surface_reader_test.rs",
        "TestNetworkRoaming",
        "network_calls",
        "first.network_roaming.active_interface",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/network_roaming.rs",
        "DesktopNetworkRoamingPort",
        "default_routes",
        "parse_linux_default_routes",
        "parse_macos_default_route",
        "parse_windows_default_routes",
        "route anchor",
        "spawn_blocking",
        "parse_route_get_interface",
        "repeated_no_next_hop_repair_is_idempotent_on_one_host",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/runtime/ports.rs",
        "fn network_roaming_port",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/runtime.rs",
        "network_roaming_port",
        "DesktopNetworkRoamingPort",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/composition.rs",
        "NetworkRoamingApplication",
        "with_network_roaming",
        "DesktopNetworkRoamingPort",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/surface.rs",
        "NetworkRoamingApplication",
        ".with_network_roaming",
        "Capability::NetworkRoaming",
    )
    require(
        violations,
        "crates/infiltrator-ports/src/host_runtime.rs",
        "network_roaming_port",
        "NetworkRoamingPort",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/types/runtime.rs",
        "pub type NetworkRoamingState",
        "NetworkRoamingSnapshot",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/types/message.rs",
        "NetworkInterfacesPolled(infiltrator_contract::network_roaming::NetworkRoamingSnapshot)",
        "NetworkRoamingRepaired(",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update/ui_wave4.rs",
        "NetworkRoamingApplication::new",
        "refresh_from",
        "force_repair()",
        "demo_network_roaming_snapshot",
        "NetworkRoamingRepaired",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/net_roam_card.rs",
        "NetworkRoamingStatus",
        'roam.active_interface.as_deref().unwrap_or("—")',
        'roam.default_gateway.as_deref().unwrap_or("—")',
        "roam.last_event",
        "format_status",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/iced_six_advancements_wave4_tests.rs",
        "test_advancement_w4_1_network_roaming_and_gateway_recovery",
        "test_live_network_roaming_snapshot_updates_the_iced_projection_without_fallbacks",
        "NetworkInterfacesPolled(snapshot.clone())",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/command.rs",
        "RefreshNetworkRoaming",
        "RepairNetworkRoutes",
        "CommandIntent::RefreshNetworkRoaming",
        "CommandIntent::RepairNetworkRoutes",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/settings_network_roaming.rs",
        "NetworkRoamingRefreshButton",
        "NetworkRoamingRepairButton",
        "NetworkRoamingStatusLine",
        "NetworkRoamingInterfacesLine",
        "NetworkRoamingRouteLine",
        "UiCommand::RefreshNetworkRoaming",
        "UiCommand::RepairNetworkRoutes",
        "SettingsProjectionUpdated",
        "format_status",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/settings.rs",
        "settings_network_roaming::scene",
        "settings_network_roaming::on_action_activated",
        "settings_network_roaming::apply_projection",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/settings_core.rs",
        "pub network_roaming: infiltrator_contract::network_roaming::NetworkRoamingSnapshot",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/pages_matrix_b_tests.rs",
        "test_settings_network_roaming_projection_and_actions_submit_shared_commands",
        "NetworkRoamingRefreshButton",
        "NetworkRoamingRepairButton",
        "UiCommand::RefreshNetworkRoaming",
        "UiCommand::RepairNetworkRoutes",
    )
    require(
        violations,
        "crates/infiltrator-android/src/runtime.rs",
        "Capability::NetworkRoaming",
        "Android native VpnService route callbacks are not exposed",
    )
    require(
        violations,
        "crates/infiltrator-ios/src/lib.rs",
        "Capability::NetworkRoaming",
        "iOS NetworkExtension route callbacks are not exposed",
    )

    if violations:
        for violation in violations:
            print(f"network-roaming-guard: {violation}", file=sys.stderr)
        print(f"network-roaming-guard: violations={len(violations)}", file=sys.stderr)
        return 1 if args.mode == "enforce" else 0
    print("network-roaming-guard: DUAL-02-12 markers=complete violations=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
