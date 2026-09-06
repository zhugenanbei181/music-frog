#!/usr/bin/env python3
"""Fail-closed guard for DUAL-02-06 system-proxy crash recovery."""

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
        "`DUAL-02-06` 非正常断电/死机系统代理自愈恢复",
        "owner PID/启动时间",
        "durable atomic journal",
        "previous",
        "desired target",
        "`parity-ready`",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/system_proxy.rs",
        "SystemProxyRecoveryReport",
        "SystemProxyRecoveryStatus",
        "SystemProxyRecoverySnapshot",
        "SkippedExternal",
        "SkippedLiveOwner",
        "Failed { failure: Failure }",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/surface_snapshot.rs",
        "pub system_proxy_recovery: SystemProxyRecoverySnapshot",
    )
    require(
        violations,
        "crates/infiltrator-ports/src/system_proxy.rs",
        "shared_recovery",
        "arm_recovery",
        "recover_orphaned",
        "clear_recovery",
    )
    require(
        violations,
        "crates/infiltrator-application/src/system_proxy_application.rs",
        "pub fn recovery_snapshot",
        "pub async fn recover_orphaned",
        "pub async fn clear_recovery",
        "arm_recovery",
        "if !enabled",
        "clear_recovery().await",
        "disabling_proxy_releases_ownership_and_clears_recovery",
        "application_publishes_typed_startup_recovery_result",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/system_proxy.rs",
        "RecoveryJournal",
        "owner_start_time",
        "write_journal",
        "file.sync_all()",
        "atomic_replace",
        "owner_process_is_live",
        "recover_orphaned_sync",
        "restore_and_clear_sync",
        "recovery_journal_round_trips_and_keeps_original_state",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/exit_cleanup.rs",
        "restore_and_clear_sync",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/runtime.rs",
        "SystemProxyApplication",
        "recover_orphaned().await",
        "SystemProxyRecoveryStatus::NotNeeded",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/app.rs",
        "Message::SystemProxyRecoveryFinished",
        "application.recover_orphaned().await",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update/system_proxy.rs",
        "finish_system_proxy_recovery",
        "SystemProxyRecoveryStatus::Restored",
        "SystemProxyRecoveryStatus::SkippedExternal",
        "系统代理启动恢复失败",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/admin_server.rs",
        "SystemProxyApplication::new",
        ".set_enabled(false, None, None)",
        ".set_enabled(true, Some(endpoint), None)",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update/core/kernels.rs",
        "system_proxy_application",
        ".set_enabled(false, None, None)",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/iced_six_advancements_wave5_tests.rs",
        "test_system_proxy_recovery_updates_the_iced_projection",
        "SystemProxyRecoveryStatus::Restored",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/surface_projection.rs",
        "system_proxy_recovery: snapshot.system_proxy_recovery.clone()",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/settings_system.rs",
        "SystemProxyRecoveryStatus",
        "format_status",
        "启动已清理孤儿代理",
        "检测到外部修改，未覆盖",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/pages_matrix_b_tests.rs",
        "test_settings_system_proxy_recovery_status_projects_shared_result",
        "SystemProxyRecoveryStatus::Restored",
        "检测到外部修改，未覆盖",
    )
    require(
        violations,
        "crates/infiltrator-android/src/runtime.rs",
        "Capability::SystemProxy",
        "Android VpnService owns proxy routing",
    )
    require(
        violations,
        "crates/infiltrator-ios/src/lib.rs",
        "Capability::SystemProxy",
        "iOS has no global proxy API",
    )

    if violations:
        for violation in violations:
            print(f"system-proxy-recovery-guard: {violation}", file=sys.stderr)
        print(
            f"system-proxy-recovery-guard: violations={len(violations)}",
            file=sys.stderr,
        )
        return 1 if args.mode == "enforce" else 0
    print("system-proxy-recovery-guard: DUAL-02-06 markers=complete violations=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
