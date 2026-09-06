#!/usr/bin/env python3
"""Fail-closed guard for DUAL-02-13 Android VpnService lifecycle."""

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
        "DUAL-02-13",
        "Android VpnService 移动端无缝穿透",
        "VpnStartRequest/VpnSessionSnapshot",
        "API 26–35",
        "typed unsupported",
        "parity-ready",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/vpn.rs",
        "MIN_VPN_MTU_BYTES",
        "MAX_VPN_MTU_BYTES",
        "VpnRoute",
        "VpnConfiguration",
        "VpnStartRequest",
        "VpnSessionState",
        "PermissionRequired",
        "Revoked",
        "VpnSessionSnapshot",
        "foreground",
        "is_running",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/capability.rs",
        "VpnService",
    )
    require(
        violations,
        "crates/infiltrator-domain/src/vpn_policy.rs",
        "validate_start_request",
        "validate_configuration",
        "tun_fd",
        "foreground_requested",
        "route_and_dns_values_are_validated_as_ip_literals",
    )
    require(
        violations,
        "crates/infiltrator-ports/src/vpn_service.rs",
        "trait VpnServicePort",
        "async fn request_start",
        "async fn prepare",
        "async fn start",
        "async fn stop",
        "async fn revoke",
        "async fn snapshot",
    )
    require(
        violations,
        "crates/infiltrator-application/src/vpn_application.rs",
        "pub struct VpnServiceApplication",
        "pub async fn request_start",
        "pub async fn prepare",
        "pub async fn start",
        "pub async fn stop",
        "pub async fn revoke",
        "VpnService start readback mismatch",
        "application_requires_foreground_running_readback",
        "invalid_fd_is_rejected_before_host_start",
        "VpnService revoke readback mismatch",
    )
    require(
        violations,
        "crates/infiltrator-application/src/command_application.rs",
        "with_vpn",
        "CommandIntent::StartVpn",
        "CommandIntent::StopVpn",
        "self.vpn()?.request_start()",
        "self.vpn()?.stop()",
    )
    require(
        violations,
        "crates/infiltrator-application/src/surface_reader.rs",
        "with_vpn",
        "read_vpn",
        "vpn,",
    )
    require(
        violations,
        "crates/infiltrator-application/src/surface_reader_services.rs",
        "read_vpn",
        "Capability::VpnService",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/surface_snapshot.rs",
        "pub vpn: crate::vpn::VpnSessionSnapshot",
    )
    require(
        violations,
        "crates/infiltrator-application/src/surface_reader_test.rs",
        "TestVpn",
        "first.vpn.is_running()",
        "first.vpn.route_count",
    )
    require(
        violations,
        "crates/mihomo-platform/src/android_bridge.rs",
        "vpn_apply_configuration",
        "vpn_is_foreground",
        "VpnService.prepare",
        "foreground",
    )
    require(
        violations,
        "crates/infiltrator-android/src/jni_bridge.rs",
        "vpnApplyConfiguration",
        "vpnIsForeground",
        "SIG_STRING_BOOL",
    )
    require(
        violations,
        "crates/infiltrator-android/src/vpn_service.rs",
        "AndroidVpnServicePort",
        "VpnServicePort",
        "async fn prepare",
        "vpn_apply_configuration",
        "tun2proxy::mobile_run",
        "store_worker_exit",
        "vpn_is_foreground",
        "non_android_host_returns_typed_vpn_unsupported",
    )
    require(
        violations,
        "crates/infiltrator-android/src/uniffi_api/vpn.rs",
        "pub fn start_vpn",
        "pub fn prepare_vpn",
        "VpnServiceApplication",
        "build_vpn_configuration",
        "build_vpn_start_request",
        "pub fn stop_vpn",
        "pub fn revoke_vpn",
        "VpnSessionSnapshot",
        "vpn_session_status",
        "map_vpn_snapshot",
        "foreground Running",
    )
    require(
        violations,
        "crates/infiltrator-android/src/infiltrator_android.udl",
        "VpnSessionResult vpn_session_status",
        "FfiStatus revoke_vpn",
        "FfiStatus prepare_vpn",
        "dictionary VpnSessionSnapshot",
        "route_count",
    )
    require(
        violations,
        "crates/infiltrator-android/src/composition.rs",
        "VpnServiceApplication",
        "AndroidVpnServicePort",
        "with_vpn",
    )
    require(
        violations,
        "crates/infiltrator-android/src/runtime.rs",
        "Capability::VpnService",
        "supports(Capability::VpnService)",
    )
    require(
        violations,
        "crates/infiltrator-android/README.md",
        "VpnService lifecycle contract",
        "VpnService.prepare",
        "prepare_vpn()",
        "Builder",
        "start_vpn(fd)",
        "onRevoke",
        "vpn_session_status",
    )
    require(
        violations,
        "android/app/src/main/java/com/musicfrog/infiltrator/RustBridge.kt",
        "vpnApplyConfiguration",
        "vpnIsForeground",
    )
    require(
        violations,
        "android/app/src/main/java/com/musicfrog/infiltrator/MihomoHost.kt",
        "override fun vpnApplyConfiguration",
        "override fun vpnIsForeground",
        "MihomoVpnService.isRunning",
        "STOPPING",
    )
    require(
        violations,
        "android/app/src/main/java/com/musicfrog/infiltrator/MihomoVpnService.kt",
        "prepareVpn()",
        "startVpn(fd)",
        "revokeVpn()",
        "Builder",
        "excludeRoute",
        "onRevoke",
        "setPendingConfiguration",
        "interfaceActive",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/host.rs",
        "Capability::VpnService",
        "Android VpnService is a mobile-host capability",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/surface.rs",
        "Capability::VpnService",
        "Android VpnService is a mobile-host capability",
    )
    require(
        violations,
        "crates/infiltrator-ios/src/lib.rs",
        "Capability::VpnService",
        "iOS NetworkExtension VPN host is not wired",
    )
    require(
        violations,
        "crates/infiltrator-ports/src/host_runtime.rs",
        "vpn_service_port",
        "VpnServicePort",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/state.rs",
        "pub vpn: infiltrator_contract::vpn::VpnSessionSnapshot",
        "snapshot.vpn",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/types/message.rs",
        "StartVpn",
        "StopVpn",
        "VpnSessionUpdated(Result<",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update/ui_wave4.rs",
        "fn apply_vpn",
        "VpnServiceApplication::new",
        "request_start()",
        "Message::VpnSessionUpdated",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/vpn_card.rs",
        "VpnSessionState",
        "Message::StartVpn",
        "Message::StopVpn",
        "foreground=",
        "format_status",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/iced_six_advancements_wave4_tests.rs",
        "test_vpn_session_snapshot_updates_the_iced_projection",
        "VpnSessionSnapshot::running",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/command.rs",
        "StartVpn",
        "StopVpn",
        "CommandIntent::StartVpn",
        "CommandIntent::StopVpn",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/settings_vpn.rs",
        "VpnStartButton",
        "VpnStopButton",
        "VpnStatusLine",
        "UiCommand::StartVpn",
        "UiCommand::StopVpn",
        "format_status",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/settings.rs",
        "settings_vpn::scene",
        "settings_vpn::on_action_activated",
        "settings_vpn::apply_projection",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/settings_core.rs",
        "pub vpn: infiltrator_contract::vpn::VpnSessionSnapshot",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/pages_matrix_b_tests.rs",
        "test_settings_vpn_projection_and_actions_submit_shared_commands",
        "VpnStartButton",
        "VpnStopButton",
        "UiCommand::StartVpn",
        "UiCommand::StopVpn",
    )

    if violations:
        for violation in violations:
            print(f"vpn-service-guard: {violation}", file=sys.stderr)
        print(f"vpn-service-guard: violations={len(violations)}", file=sys.stderr)
        return 1 if args.mode == "enforce" else 0
    print("vpn-service-guard: DUAL-02-13 markers=complete violations=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
