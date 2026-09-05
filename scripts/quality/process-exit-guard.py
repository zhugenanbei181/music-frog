#!/usr/bin/env python3
"""Fail-closed guard for DUAL-01-10 process-exit cleanup."""

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
        "`DUAL-01-10` 进程退出清理保证",
        "SIGINT/SIGTERM/Ctrl+C",
        "`parity-ready`",
    )
    require(
        violations,
        "crates/mihomo-platform/src/crash_reporter.rs",
        "TerminationHandler",
        "install_termination_handlers",
        "SIGINT",
        "SIGTERM",
        "run_emergency_cleanup",
        "std::process::exit(128 + signal)",
    )
    require(
        violations,
        "crates/mihomo-platform/src/crash_reporter_test.rs",
        "test_clean_exit_hook_registration_and_execution",
        "run_emergency_cleanup",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/exit_cleanup.rs",
        "pub fn install",
        "register_proxy_restore",
        "register_tun_route_restore",
        "pub fn run_now",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/desktop_composition.rs",
        "exit_cleanup::install",
        "exit_cleanup::run_now",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update/ui.rs",
        "if let Some(cleanup) = &self.exit_cleanup",
        "Message::Exit",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/lib.rs",
        "host-composed application command service",
        "host owns process/VPN adapters",
    )
    require(
        violations,
        "crates/mihomo-platform/src/desktop.rs",
        "PR_SET_PDEATHSIG",
        "parent_death_signal",
    )

    if violations:
        for violation in violations:
            print(f"process-exit-guard: {violation}", file=sys.stderr)
        print(f"process-exit-guard: violations={len(violations)}", file=sys.stderr)
        return 1 if args.mode == "enforce" else 0
    print("process-exit-guard: DUAL-01-10 markers=complete violations=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
