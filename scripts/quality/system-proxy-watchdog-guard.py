#!/usr/bin/env python3
"""Fail-closed guard for DUAL-02-05 system-proxy ownership watchdog."""

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
        "`DUAL-02-05` 系统代理被抢占实时探活",
        "3 秒",
        "ownership",
        "`parity-ready`",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/system_proxy.rs",
        "SystemProxyDesiredState",
        "SystemProxyOwnership",
        "Repaired",
        "repair_count",
        "with_repaired",
    )
    require(
        violations,
        "crates/infiltrator-ports/src/system_proxy.rs",
        "shared_target",
        "SystemProxyDesiredState",
    )
    require(
        violations,
        "crates/infiltrator-application/src/system_proxy_application.rs",
        "desired",
        "repair_count",
        "reconcile_fresh",
        "target_matches",
        "application_repairs_external_override_and_reports_one_warning_state",
        "separate_compositions_share_proxy_ownership_through_the_port",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/system_proxy.rs",
        "SHARED_SYSTEM_PROXY_TARGET",
        "fn shared_target",
    )
    require(
        violations,
        "crates/infiltrator-application/src/surface_reader_services.rs",
        "snapshot_cached",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/types/message.rs",
        "SystemProxyReconciled",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/subscription.rs",
        "system_proxy_watchdog_subscription",
        "Duration::from_secs(3)",
        "build_system_proxy_watchdog",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/state.rs",
        "system_proxy_application",
        "system_proxy_last_repair_count",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update/system_proxy.rs",
        "SystemProxyOwnership::Repaired",
        "系统代理设置被其他程序修改，已自动恢复",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/settings_system.rs",
        "SystemProxyOwnership::Repaired",
        "已自动修复外部修改",
        "format_status",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/settings_tun.rs",
        "SystemProxyToggle",
        "projection.system_proxy",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/iced_six_advancements_wave5_tests.rs",
        "test_shared_surface_system_proxy_updates_the_iced_projection",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/pages_matrix_b_tests.rs",
        "test_settings_system_proxy_checkbox_submits_shared_command",
        "UiCommand::SetSystemProxy { enabled: false }",
    )

    if violations:
        for violation in violations:
            print(f"system-proxy-watchdog-guard: {violation}", file=sys.stderr)
        print(
            f"system-proxy-watchdog-guard: violations={len(violations)}",
            file=sys.stderr,
        )
        return 1 if args.mode == "enforce" else 0
    print("system-proxy-watchdog-guard: DUAL-02-05 markers=complete violations=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
