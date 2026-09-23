#!/usr/bin/env python3
"""Fail-closed guard for DUAL-02-04 system HTTP/SOCKS proxy injection."""

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
        "`DUAL-02-04` 系统 HTTP/SOCKS 代理一键注入",
        "Windows registry",
        "Linux GNOME/KDE/GSettings",
        "macOS networksetup",
        "`parity-ready`",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/system_proxy.rs",
        "SystemProxyObservation",
        "SystemProxySnapshot",
        "SystemProxyStatus",
        "from_observation",
        "is_enabled",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/surface_snapshot.rs",
        "pub system_proxy: SystemProxySnapshot",
    )
    require(
        violations,
        "crates/infiltrator-ports/src/system_proxy.rs",
        "pub trait SystemProxyPort",
        "async fn snapshot",
        "async fn apply",
    )
    require(
        violations,
        "crates/infiltrator-application/src/system_proxy_application.rs",
        "SYSTEM_PROXY_SNAPSHOT_CACHE_TTL",
        "pub async fn snapshot_cached",
        "pub async fn set_enabled",
        "system proxy readback mismatch",
        "application_applies_and_reads_back_proxy_state",
    )
    require(
        violations,
        "crates/infiltrator-application/src/surface_reader.rs",
        "with_system_proxy",
        "read_system_proxy",
    )

    require(
        violations,
        "crates/infiltrator-application/src/surface_reader/page_builders.rs",
        "system_proxy.is_enabled()",
    )
    require(
        violations,
        "crates/infiltrator-application/src/command_application/dispatch.rs",
        "CommandIntent::SetSystemProxy",
        "system_proxy()",
        "set_enabled(enabled, endpoint, None)",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/system_proxy.rs",
        "DesktopSystemProxy",
        "apply_system_proxy_with_bypass",
        "read_system_proxy_state",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/proxy/windows.rs",
        "ProxyEnable",
        "ProxyServer",
        "ProxyOverride",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/proxy/linux/gnome.rs",
        "org.gnome.system.proxy",
        "manual",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/proxy/linux/kde.rs",
        "kwriteconfig",
        "socksProxy",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/proxy/macos.rs",
        "networksetup",
        "-setwebproxy",
        "-setsocksfirewallproxy",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/runtime.rs",
        "fn system_proxy_port",
        "DesktopSystemProxy::new",
    )
    require(
        violations,
        "crates/infiltrator-ports/src/host_runtime.rs",
        "fn system_proxy_port",
        "Option<Arc<dyn SystemProxyPort>>",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/composition.rs",
        "with_system_proxy",
        "SystemProxyApplication",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/surface.rs",
        "with_system_proxy(system_proxy)",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/types/message.rs",
        "SystemProxySet(",
        "SystemProxySnapshot",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/state.rs",
        "pub system_proxy: SystemProxySnapshot",
        "pub system_proxy_port: Option<Arc<dyn SystemProxyPort>>",
        "snapshot.system_proxy",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update/system_proxy.rs",
        "proxy_application",
        ".set_enabled(",
        "Message::SystemProxySet",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/iced_six_advancements_wave5_tests.rs",
        "test_shared_surface_system_proxy_updates_the_iced_projection",
        "SystemProxyStatus::Enabled",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/command.rs",
        "SetSystemProxy { enabled: true }",
        "CommandIntent::SetSystemProxy",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/settings_system.rs",
        "SystemProxyToggle",
        "toggle_scene",
        "on_changed",
        "UiCommand::SetSystemProxy",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/settings_tun.rs",
        "SystemProxyToggle",
        "projection.system_proxy",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/pages_matrix_b_tests.rs",
        "test_settings_system_proxy_checkbox_submits_shared_command",
        "UiCommand::SetSystemProxy { enabled: false }",
    )

    if violations:
        for violation in violations:
            print(f"system-proxy-guard: {violation}", file=sys.stderr)
        print(f"system-proxy-guard: violations={len(violations)}", file=sys.stderr)
        return 1 if args.mode == "enforce" else 0
    print("system-proxy-guard: DUAL-02-04 markers=complete violations=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
