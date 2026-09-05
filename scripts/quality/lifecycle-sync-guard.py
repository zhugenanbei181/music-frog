#!/usr/bin/env python3
"""Fail-closed guard for DUAL-01-14 lifecycle parity."""

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
        "`DUAL-01-14` 双端生命周期状态机同步",
        "CoreLifecycleSnapshot",
        "`parity-ready`",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/snapshot.rs",
        "pub struct CoreLifecycleSnapshot",
        "lifecycle_snapshot(&self)",
        "session_token",
        "revision",
    )
    require(
        violations,
        "crates/infiltrator-ports/src/core_lifecycle.rs",
        "CoreLifecycleSnapshot",
        "fn lifecycle_snapshot",
    )
    require(
        violations,
        "crates/infiltrator-application/src/core_application.rs",
        "CoreLifecycleSnapshot",
        "fn lifecycle_snapshot(&self)",
        "self.snapshot().lifecycle_snapshot()",
    )
    require(
        violations,
        "crates/infiltrator-application/src/core_application_tests.rs",
        "let lifecycle = app.lifecycle_snapshot()",
        "lifecycle.session_token",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/types/runtime.rs",
        "from_core_snapshot",
        "CoreLifecycle::Starting",
        "CoreLifecycle::Failed",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/state.rs",
        "RuntimeStatus::from_core_snapshot(&snapshot.core)",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/surface_contract_tests.rs",
        "shared_core_lifecycle_snapshot_drives_iced_status_for_every_phase",
        "RuntimeStatus::Error",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/surface.rs",
        "LatestCoreLifecycle",
        "core_lifecycle_projection",
        "CoreLifecycleSnapshot",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/route.rs",
        "LatestCoreLifecycle(core_lifecycle_projection",
        "commands.insert_resource(LatestCoreLifecycle",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/command.rs",
        "StartCore",
        "StopCore",
        "RestartCore",
        "CommandIntent::StartCore",
        "CommandIntent::StopCore",
        "CommandIntent::RestartCore",
        "lifecycle_commands_share_the_core_application_intents",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/surface_tests.rs",
        "shared_core_lifecycle_projection_tracks_session_generation_and_revision",
        "LatestCoreLifecycle",
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

    if violations:
        for violation in violations:
            print(f"lifecycle-sync-guard: {violation}", file=sys.stderr)
        print(f"lifecycle-sync-guard: violations={len(violations)}", file=sys.stderr)
        return 1 if args.mode == "enforce" else 0
    print("lifecycle-sync-guard: DUAL-01-14 markers=complete violations=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
