#!/usr/bin/env python3
"""Fail-closed guard for DUAL-02-10 Windows UWP loopback controls."""

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
        "`DUAL-02-10` Windows UWP 回环隔离解除工具",
        "UwpLoopbackSnapshot",
        "CheckNetIsolation.exe",
        "typed unsupported",
        "不再用固定 UWP 列表冒充扫描结果",
        "`parity-ready`",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/uwp.rs",
        "UwpPackageSnapshot",
        "UwpLoopbackAvailability",
        "UwpLoopbackSnapshot",
        "empty successful package list",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/command.rs",
        "ScanUwpApps",
        "SetUwpAppExemption { sid: String, exempt: bool }",
        "SetAllUwpExemptions { exempt: bool }",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/capability.rs",
        "UwpLoopback",
    )
    require(
        violations,
        "crates/infiltrator-domain/src/uwp.rs",
        "validate_app_container_sid",
        "S-1-15-2",
        "injection",
    )
    require(
        violations,
        "crates/infiltrator-ports/src/uwp_loopback.rs",
        "trait UwpLoopbackPort",
        "async fn scan",
        "async fn set_exempt",
        "async fn set_all",
    )
    require(
        violations,
        "crates/infiltrator-application/src/uwp_loopback_application.rs",
        "pub async fn snapshot",
        "pub async fn set_exempt",
        "pub async fn set_all",
        "validate_app_container_sid",
        "verify_package_state",
        "application_scans_and_reads_back_single_exemption",
        "application_rejects_ignored_exemption_write",
    )
    require(
        violations,
        "crates/infiltrator-application/src/command_application.rs",
        "with_uwp_loopback",
        "CommandIntent::ScanUwpApps",
        "CommandIntent::SetUwpAppExemption",
        "CommandIntent::SetAllUwpExemptions",
    )
    require(
        violations,
        "crates/infiltrator-application/src/surface_reader.rs",
        "with_uwp_loopback",
        "read_uwp_loopback",
        "app_routing_page(",
        "uwp_loopback",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/surface_snapshot.rs",
        "pub uwp_loopback: crate::uwp::UwpLoopbackSnapshot",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/uwp_loopback.rs",
        "try_list_app_containers",
        "self.set_exempt(&container.sid, true)?",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/uwp_loopback_port.rs",
        "DesktopUwpLoopbackPort",
        "CheckNetIsolation",
        "cfg!(windows)",
        "UwpLoopbackPort",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/surface.rs",
        "UwpLoopbackApplication",
        ".with_uwp_loopback",
        "Capability::UwpLoopback",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/composition.rs",
        "with_uwp_loopback",
        "DesktopUwpLoopbackPort",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/host.rs",
        "uwp_loopback_application",
        "DesktopUwpLoopbackPort",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/state.rs",
        "uwp_loopback.availability",
        "app_routing.uwp_loopback",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update/ui_wave3.rs",
        "map_uwp_snapshot",
        "!self.shell.demo",
        "application.snapshot().await",
        ".set_all(",
        ".set_exempt(",
        "UwpExemptionsChanged",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/types/message.rs",
        "UwpSnapshotLoaded",
        "UwpExemptionsChanged",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/uwp_card.rs",
        "UwpLoopbackAvailability",
        "status_message",
        "ToggleUwpAppExemption",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/iced_six_advancements_wave3_tests.rs",
        "test_uwp_live_snapshot_projects_without_demo_data",
        "UwpSnapshotLoaded",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/app_routing.rs",
        "uwp_exemption_scene(projection, palette)",
        "app_routing_uwp::on_action_activated",
        "app_routing_uwp::apply_projection",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/app_routing_uwp.rs",
        "UwpPackageSnapshot",
        "UwpActionButton",
        "UiCommand::ScanUwpApps",
        "UiCommand::SetUwpAppExemption",
        "UiCommand::SetAllUwpExemptions",
        "On<AppRoutingProjectionUpdated>",
        "format_status",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/command.rs",
        "ScanUwpApps",
        "SetUwpAppExemption",
        "SetAllUwpExemptions",
        "CommandIntent::ScanUwpApps",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/pages_matrix_b_tests.rs",
        "test_app_routing_uwp_actions_submit_shared_commands",
        "UwpAction::ExemptAll",
        "SetAllUwpExemptions { exempt: true }",
    )
    require(
        violations,
        "crates/infiltrator-android/src/runtime.rs",
        "Capability::UwpLoopback",
        "Windows AppContainer loopback is not available on Android",
    )
    require(
        violations,
        "crates/infiltrator-ios/src/lib.rs",
        "Capability::UwpLoopback",
        "Windows AppContainer loopback is not available on iOS",
    )

    if violations:
        for violation in violations:
            print(f"uwp-loopback-guard: {violation}", file=sys.stderr)
        print(f"uwp-loopback-guard: violations={len(violations)}", file=sys.stderr)
        return 1 if args.mode == "enforce" else 0
    print("uwp-loopback-guard: DUAL-02-10 markers=complete violations=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
