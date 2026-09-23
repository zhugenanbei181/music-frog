#!/usr/bin/env python3
"""Fail-closed guard for DUAL-03-09 Overview speedtest button parity."""

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
        "DUAL-03-09",
        "全局一键并发测速按钮",
        "parity-ready",
    )
    require(
        violations,
        "docs/CORE_030_REARCHITECTURE.md",
        "DUAL-03-09",
        "TestAllProxyGroups",
        "TestAllProxyDelays",
    )
    require(
        violations,
        "docs/DUAL_SURFACE_ARCHITECTURE_AUDIT_030.md",
        "DUAL-03-09",
        "OverviewSpeedtestButton",
        "host-verified",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/overview_speedtest.rs",
        "OverviewSpeedtestButton",
        "OverviewSpeedtestText",
        "on_overview_speedtest_activated",
        "UiCommand::TestAllProxyGroups",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/overview_tests.rs",
        "overview_speedtest_button_submits_test_all_proxy_groups",
        "UiCommand::TestAllProxyGroups",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/overview.rs",
        "overview_speedtest_button",
        "Message::TestAllProxyDelays",
    )
    require(
        violations,
        "scripts/test.sh",
        "overview-speedtest-guard.py --mode enforce",
    )
    require(
        violations,
        "scripts/test-bevy.sh",
        "overview-speedtest-guard.py\" --mode enforce",
    )

    if violations:
        for violation in violations:
            print(f"overview-speedtest-guard: {violation}", file=sys.stderr)
        print(
            f"overview-speedtest-guard: violations={len(violations)}",
            file=sys.stderr,
        )
        return 1 if args.mode == "enforce" else 0
    print("overview-speedtest-guard: DUAL-03-09 markers=complete violations=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
