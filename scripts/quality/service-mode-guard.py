#!/usr/bin/env python3
"""Fail-closed guard for DUAL-01-09 privileged service-mode parity."""

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
        "`DUAL-01-09` 服务模式",
        "Windows Service",
        "Linux Polkit",
        "macOS launchd",
        "`parity-ready`",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/service_mode.rs",
        "ServiceModePlatform",
        "WindowsService",
        "LinuxPolkit",
        "MacosLaunchd",
        "ServiceModeSnapshot",
    )
    require(
        violations,
        "crates/infiltrator-ports/src/service_mode.rs",
        "trait ServiceModePort",
        "async fn snapshot",
        "async fn prepare",
    )
    require(
        violations,
        "crates/infiltrator-application/src/service_mode_application.rs",
        "pub async fn snapshot",
        "pub async fn prepare",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/service_mode.rs",
        "DesktopServiceMode",
        "TunServiceManager::install_service",
        "TunServiceManager::start_service",
        "did not reach ready state",
        "current_platform",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/tun_service/linux.rs",
        "pkexec",
        "setcap_argv",
        "test_setcap_argv_uninstall_exact_inverse",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/tun_service/windows.rs",
        "sc.exe",
        "install_service",
        "start_service",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/tun_service/macos.rs",
        "UnsupportedPlatformError",
        "test_tun_macos_verbs_return_typed_unsupported",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/service/macos.rs",
        "launchd",
        "MacPrivilegedHelperContract",
        "generate_launchd_plist",
    )
    require(
        violations,
        "crates/infiltrator-ports/src/host_runtime.rs",
        "service_mode_port",
        "Option<Arc<dyn ServiceModePort>>",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/runtime.rs",
        "DesktopServiceMode",
        "fn service_mode_port",
    )
    require(
        violations,
        "crates/infiltrator-application/src/surface_reader.rs",
        "with_service_mode",
        "read_service_mode",
        "service_mode,",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/surface.rs",
        "ServiceModeApplication",
        "DesktopServiceMode",
        ".with_service_mode(service_mode)",
    )
    require(
        violations,
        "crates/infiltrator-application/src/command_application.rs",
        "with_service_mode",
        "CommandIntent::PrepareServiceMode",
        "self.service_mode()?.prepare()",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/composition.rs",
        "with_service_mode(service_mode)",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update/core/runtime_config.rs",
        "service_mode_port",
        "ServiceModePrepared",
        "legacy_service_snapshot",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/state.rs",
        "service_mode: ServiceModeSnapshot",
        "runtime.service_mode = snapshot.service_mode",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/settings.rs",
        "format_service_mode",
        "Service mode",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/command.rs",
        "PrepareServiceMode",
        "CommandIntent::PrepareServiceMode",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/settings.rs",
        "ServiceModeButton",
        "ServiceModeAvailability",
        "SettingsLineKind::ServiceMode",
        "UiCommand::PrepareServiceMode",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/settings_core.rs",
        "service_mode_row_scene",
        "format_service_mode",
    )
    require(
        violations,
        "crates/infiltrator-application/src/surface_reader.rs",
        "TestServiceMode",
        "ServiceModeState::Ready",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/surface_contract_tests.rs",
        "ServiceModeState::Ready",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/pages_matrix_tests.rs",
        "ServiceModeState::Ready",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/pages_matrix_b_tests.rs",
        "test_settings_prepare_tun_and_toggles_submit_commands",
        "UiCommand::PrepareServiceMode",
    )

    if violations:
        for violation in violations:
            print(f"service-mode-guard: {violation}", file=sys.stderr)
        print(f"service-mode-guard: violations={len(violations)}", file=sys.stderr)
        return 1 if args.mode == "enforce" else 0
    print("service-mode-guard: DUAL-01-09 markers=complete violations=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
