#!/usr/bin/env python3
"""Fail-closed guard for DUAL-01-15 headless lifecycle coverage."""

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
        "`DUAL-01-15` 双端无头测试全景覆盖",
        "失败启动、端口冲突与平滑停止",
        "`parity-ready`",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/snapshot.rs",
        "CoreLifecycleSnapshot",
        "CoreLifecycle",
        "session_token",
    )
    require(
        violations,
        "crates/infiltrator-application/src/core_application_tests.rs",
        "lifecycle_commands_publish_only_contract_values",
        "readiness_failures_never_report_running",
        "CoreLifecycle::Failed",
        "CoreLifecycle::Stopped",
    )
    require(
        violations,
        "crates/infiltrator-application/src/port_conflict_application.rs",
        "PortConflictApplication",
        "pub async fn snapshot",
        "pub async fn repair",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/business_flow/runtime_tray_kernels.rs",
        "dual_surface_headless_lifecycle_matrix_covers_failure_conflict_and_stop",
        "local core refused to start",
        "unrelated-app",
        "Message::ProxyStopped",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/business_flow/runtime_tray_kernels.rs",
        "port_conflict_repair_replaces_the_shared_snapshot_without_killing_unknown_pids",
        "can_release: false",
        "9090",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/surface_tests.rs",
        "dual_surface_headless_lifecycle_matrix_covers_failure_conflict_and_stop",
        "simulated core start failure",
        "unrelated-app",
        "CoreLifecycle::Stopped",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/command.rs",
        "lifecycle_commands_share_the_core_application_intents",
        "Some(CommandIntent::StartCore)",
        "Some(CommandIntent::StopCore)",
        "Some(CommandIntent::RestartCore)",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/surface.rs",
        "LatestCoreLifecycle",
        "SurfaceSnapshotUpdated",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update/core/lifecycle.rs",
        "Message::ProxyStarted",
        "Message::ProxyStopped",
        "ManagedRuntime::shutdown",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/state.rs",
        "RuntimeStatus::from_core_snapshot",
        "core_lifecycle",
    )

    if violations:
        for violation in violations:
            print(f"lifecycle-matrix-guard: {violation}", file=sys.stderr)
        print(f"lifecycle-matrix-guard: violations={len(violations)}", file=sys.stderr)
        return 1 if args.mode == "enforce" else 0
    print("lifecycle-matrix-guard: DUAL-01-15 markers=complete violations=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
