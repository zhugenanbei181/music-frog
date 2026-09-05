#!/usr/bin/env python3
"""Fail-closed guard for DUAL-01-06 local core-version rollback parity."""

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
        "`DUAL-01-06` 内核版本秒级回滚",
        "`parity-ready`",
    )
    require(
        violations,
        "crates/mihomo-version/src/manager.rs",
        "VersionRollbackInfo",
        "pub async fn rollback_info",
        "pub async fn rollback",
        "version_history",
        "atomic_write",
        "smoke_check_binary",
        "version_selection_preserves_profile_and_rolls_back_without_network",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/version.rs",
        "CoreRollbackSnapshot",
        "pub rollback: CoreRollbackSnapshot",
    )
    require(
        violations,
        "crates/infiltrator-ports/src/version.rs",
        "async fn rollback",
        "async fn rollback_snapshot",
    )
    require(
        violations,
        "crates/infiltrator-core/src/version_port.rs",
        "async fn rollback(&self)",
        "async fn rollback_snapshot(&self)",
        "CoreRollbackSnapshot",
    )
    require(
        violations,
        "crates/infiltrator-application/src/version_application.rs",
        "pub async fn rollback",
        "self.port.rollback_snapshot()",
        "rollback_is_forwarded_through_the_application_facade",
    )
    require(
        violations,
        "crates/infiltrator-application/src/surface_reader.rs",
        "CoreRollbackSnapshot",
        "rollback_snapshot",
        "first.versions.rollback.target",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/command.rs",
        "RollbackCore",
        "Self::RollbackCore",
    )
    require(
        violations,
        "crates/infiltrator-application/src/command_application.rs",
        "CommandIntent::RollbackCore",
        "self.versions()?.rollback()",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/composition.rs",
        "CommandApplication",
        "with_versions(versions)",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/types/message.rs",
        "RollbackCore",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update/core/kernels.rs",
        "Message::RollbackCore",
        ".rollback()",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/settings.rs",
        "core_versions.rollback.target",
        "Message::RollbackCore",
        "Rollback target",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/business_flow/runtime_tray_kernels.rs",
        "kernel_rollback_round_trip_uses_the_shared_operation_path",
        "Message::RollbackCore",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/command.rs",
        "RollbackCore",
        "Some(CommandIntent::RollbackCore)",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/settings.rs",
        "CoreRollbackButton",
        "CoreRollbackAvailability",
        "SettingsLineKind::CoreRollback",
        "UiCommand::RollbackCore",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/pages_matrix_b_tests.rs",
        "test_settings_core_rollback_button_submits_shared_command",
        "UiCommand::RollbackCore",
    )

    if violations:
        for violation in violations:
            print(f"kernel-rollback-guard: {violation}", file=sys.stderr)
        print(f"kernel-rollback-guard: violations={len(violations)}", file=sys.stderr)
        return 1 if args.mode == "enforce" else 0
    print("kernel-rollback-guard: DUAL-01-06 markers=complete violations=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
