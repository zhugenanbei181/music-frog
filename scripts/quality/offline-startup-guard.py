#!/usr/bin/env python3
"""Fail-closed guard for DUAL-01-13 offline-first startup parity."""

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
        "`DUAL-01-13` 离线与无网启动容灾",
        "本地 profile YAML 与内核文件预校验",
        "offline-first",
        "`parity-ready`",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/offline_startup.rs",
        "StartupNetworkPolicy",
        "OfflineStartupSnapshot",
        "OfflineStartupState",
        "StartupRemoteDependency",
        "is_offline_startable",
    )
    require(
        violations,
        "crates/infiltrator-ports/src/offline_startup.rs",
        "trait OfflineStartupPort",
        "validate_offline_startup",
    )
    require(
        violations,
        "crates/infiltrator-application/src/offline_startup_application.rs",
        "OfflineStartupApplication",
        "validate_offline_startup",
        "OfflineStartupSnapshot::blocked",
    )
    require(
        violations,
        "crates/mihomo-config/src/yaml.rs",
        "pub fn validate",
    )
    require(
        violations,
        "crates/mihomo-config/src/manager/active.rs",
        "validate_current_profile",
        "local-only",
    )
    require(
        violations,
        "crates/mihomo-config/src/manager/manager_test.rs",
        "validate_current_profile_is_local_and_fail_closed",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/offline_startup.rs",
        "DesktopOfflineStartup",
        "validate_offline_startup",
        "No HTTP client",
        "LocalAssetStatus::Missing",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/runtime.rs",
        "bootstrap_offline",
        "ensure_geoip_database_offline",
        "without network",
        "validate_current_profile",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/boot.rs",
        "bootstrap_offline",
        "validate local profile",
        "local startup validation failed",
    )
    require(
        violations,
        "crates/infiltrator-application/src/surface_reader.rs",
        "with_offline_startup",
        "offline_startup,",
    )
    require(
        violations,
        "crates/infiltrator-application/src/surface_reader_services.rs",
        "read_offline_startup",
        "application.snapshot()",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/surface_snapshot.rs",
        "pub offline_startup: OfflineStartupSnapshot",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/surface.rs",
        "OfflineStartupApplication",
        "with_offline_startup(offline_startup)",
    )
    require(
        violations,
        "crates/infiltrator-android/src/runtime.rs",
        "OfflineStartupPort",
        "offline_startup_snapshot",
        "never performs a remote probe",
    )
    require(
        violations,
        "crates/infiltrator-android/src/composition.rs",
        "offline_startup_application",
    )
    require(
        violations,
        "crates/infiltrator-ios/src/lib.rs",
        "OfflineStartupPort",
        "offline_startup_snapshot",
        "never claims readiness without evidence",
    )
    require(
        violations,
        "crates/infiltrator-composition/src/lib.rs",
        "ios_offline_startup_application",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/state.rs",
        "offline_startup: OfflineStartupSnapshot",
        "runtime.offline_startup = snapshot.offline_startup.clone()",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/settings.rs",
        "format_offline_startup",
        "Offline startup:",
        "offline-first",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/surface_contract_tests.rs",
        "offline_startup_snapshot_updates_the_iced_runtime_projection",
        "OfflineStartupState::Degraded",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/view_settings_tests.rs",
        "test_offline_startup_status_is_rendered_without_claiming_geoip_ready",
        "geoip=missing",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/settings.rs",
        "OfflineStartup",
        "offline_startup_row_scene",
        "format_offline_startup",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/settings_core.rs",
        "离线启动 (Offline-first)",
        "format_offline_startup",
        "远端可选",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/surface_projection.rs",
        "offline_startup: snapshot.offline_startup.clone()",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/pages_matrix_tests.rs",
        "SettingsLineKind::OfflineStartup",
        "离线优先",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/pages_matrix_b_tests.rs",
        "OfflineStartupSnapshot::ready(LocalAssetStatus::Missing)",
        "可启动但已降级",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/runtime_tests.rs",
        "offline_geoip_missing_is_non_fatal_and_does_not_write_or_download",
        "ensure_geoip_database_offline",
    )

    if violations:
        for violation in violations:
            print(f"offline-startup-guard: {violation}", file=sys.stderr)
        print(f"offline-startup-guard: violations={len(violations)}", file=sys.stderr)
        return 1 if args.mode == "enforce" else 0
    print("offline-startup-guard: DUAL-01-13 markers=complete violations=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
