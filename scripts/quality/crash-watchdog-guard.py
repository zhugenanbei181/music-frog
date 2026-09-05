#!/usr/bin/env python3
"""Fail-closed guard for DUAL-01-03 managed-core crash recovery."""

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
        "crates/infiltrator-contract/src/snapshot.rs",
        "CoreWatchdogState",
        "CoreWatchdogSnapshot",
        "pub watchdog: CoreWatchdogSnapshot",
    )
    require(
        violations,
        "crates/infiltrator-domain/src/watchdog.rs",
        "WatchdogConfig",
        "delay_for_attempt",
        "max_restart_attempts",
        "process_exited",
        "restart_failed",
        "crash_recovery_trips_after_restart_failures",
    )
    require(
        violations,
        "crates/infiltrator-ports/src/core_watchdog.rs",
        "trait CoreWatchdogPort",
        "watchdog_snapshot",
        "watchdog_tick",
    )
    require(
        violations,
        "crates/infiltrator-application/src/core_watchdog.rs",
        "watchdog_tick_locked",
        "publish_watchdog_snapshot",
        "configure_watchdog",
        "impl CoreWatchdogPort for CoreApplication",
    )
    require(
        violations,
        "crates/infiltrator-application/src/core_application_tests.rs",
        "watchdog_detects_exit_and_restarts_with_a_new_session",
    )
    require(
        violations,
        "crates/infiltrator-composition/src/lib.rs",
        "CORE_WATCHDOG_POLL_INTERVAL",
        "spawn_core_watchdog",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/runtime.rs",
        "_watchdog",
        "spawn_core_watchdog",
    )
    require(
        violations,
        "crates/infiltrator-android/src/host_session.rs",
        "_watchdog",
        "spawn_core_watchdog",
    )
    require(
        violations,
        "crates/infiltrator-composition/src/lib.rs",
        "ios_core_application_with_watchdog",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/types/doctor.rs",
        "CoreWatchdogSnapshot",
        "pub shared:",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/state.rs",
        "crash_watchdog.shared = snapshot.core.watchdog.clone()",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/crash_watchdog_card.rs",
        "CoreWatchdogState",
        "Recovery pending",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/surface_contract_tests.rs",
        "shared_watchdog_snapshot_updates_the_iced_diagnostics_projection",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/doctor.rs",
        "pub watchdog: CoreWatchdogSnapshot",
        "DoctorLineKind::Watchdog",
        "watchdog_status_text",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/surface_projection.rs",
        "snapshot.core.watchdog.clone()",
        "projection.watchdog = watchdog",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/surface_tests.rs",
        "shared_watchdog_snapshot_reaches_the_bevy_doctor_projection",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/pages_matrix_b_tests.rs",
        "核心看门狗：第 2 次重启将在 200 ms 后执行",
    )

    if violations:
        for violation in violations:
            print(f"crash-watchdog-guard: {violation}", file=sys.stderr)
        print(f"crash-watchdog-guard: violations={len(violations)}", file=sys.stderr)
        return 1 if args.mode == "enforce" else 0
    print("crash-watchdog-guard: DUAL-01-03 markers=complete violations=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
