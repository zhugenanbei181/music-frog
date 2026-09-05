#!/usr/bin/env python3
"""Fail-closed guard for DUAL-01-11 safe port conflict parity."""

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
        "`DUAL-01-11` 端口冲突自动探测与避让",
        "7890/9090",
        "`parity-ready`",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/port_conflict.rs",
        "PortBinding",
        "PortConflict",
        "owner_pid",
        "can_release",
        "PortConflictSnapshot",
    )
    require(
        violations,
        "crates/infiltrator-ports/src/port_conflict.rs",
        "trait PortConflictPort",
        "async fn snapshot",
        "async fn repair",
    )
    require(
        violations,
        "crates/infiltrator-application/src/port_conflict_application.rs",
        "pub async fn snapshot",
        "pub async fn repair",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/port_conflict.rs",
        "DesktopPortConflict",
        "is_port_available",
        "owner_for_port",
        "parse_lsof_owner",
        "parse_netstat_owner",
        "never terminates an unverified third-party PID",
        "available_port_has_no_release_owner",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/storage.rs",
        "pub fn port_conflict",
        "DesktopPortConflict::new",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/surface.rs",
        "PortConflictApplication",
        ".with_port_conflicts(port_conflicts)",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/composition.rs",
        "with_port_conflicts(port_conflicts)",
    )
    require(
        violations,
        "crates/infiltrator-application/src/command_application.rs",
        "with_port_conflicts",
        "CommandIntent::RepairPortConflicts",
        "self.port_conflicts()?.repair()",
    )
    require(
        violations,
        "crates/infiltrator-application/src/surface_reader.rs",
        "with_port_conflicts",
        "read_port_conflicts",
    )
    require(
        violations,
        "crates/infiltrator-application/src/surface_reader_test.rs",
        "first.port_conflicts.has_conflicts()",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/surface_snapshot.rs",
        "pub port_conflicts: PortConflictSnapshot",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/command.rs",
        "RepairPortConflicts",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/command.rs",
        "RepairPortConflicts",
        "CommandIntent::RepairPortConflicts",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/settings.rs",
        "PortConflictButton",
        "SettingsLineKind::PortConflicts",
        "UiCommand::RepairPortConflicts",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/settings_core.rs",
        "port_conflicts_row_scene",
        "format_port_conflicts",
        "端口冲突 (Port Conflicts)",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/surface_projection.rs",
        "port_conflicts: snapshot.port_conflicts.clone()",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/pages_matrix_b_tests.rs",
        "PortConflictButton",
        "UiCommand::RepairPortConflicts",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/pages_matrix_tests.rs",
        "PortConflictSnapshot",
        "owner_pid: Some(4242)",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/state.rs",
        "port_conflicts: PortConflictSnapshot",
        "runtime.port_conflicts = snapshot.port_conflicts.clone()",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update/core/runtime_config.rs",
        "Message::RepairPortConflicts",
        "Message::PortConflictsRepaired",
        "已安全避让到可用端口",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/settings.rs",
        "format_port_conflicts",
        "Check & repair",
        "Message::RepairPortConflicts",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/business_flow/runtime_tray_kernels.rs",
        "port_conflict_repair_replaces_the_shared_snapshot_without_killing_unknown_pids",
        "unrelated-app",
    )

    if violations:
        for violation in violations:
            print(f"port-conflict-guard: {violation}", file=sys.stderr)
        print(f"port-conflict-guard: violations={len(violations)}", file=sys.stderr)
        return 1 if args.mode == "enforce" else 0
    print("port-conflict-guard: DUAL-01-11 markers=complete violations=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
