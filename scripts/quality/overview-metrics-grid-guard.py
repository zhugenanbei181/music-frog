#!/usr/bin/env python3
"""Fail-closed guard for DUAL-03-10 Overview metrics grid parity."""

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
        "DUAL-03-10",
        "核心资源 6 项运维网格",
        "parity-ready",
    )
    require(
        violations,
        "docs/CORE_030_REARCHITECTURE.md",
        "DUAL-03-10",
        "OverviewChipKind::Cpu",
        "OverviewChipKind::TotalTraffic",
    )
    require(
        violations,
        "docs/DUAL_SURFACE_ARCHITECTURE_AUDIT_030.md",
        "DUAL-03-10",
        "OverviewChipKind",
        "host-verified",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/overview.rs",
        "OverviewChipKind::Cpu",
        "OverviewChipKind::TotalTraffic",
        "format_cpu",
        "format_total_traffic",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/projection.rs",
        "pub cpu_percent: Option<f32>",
        "pub total_traffic_bytes: Option<u64>",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/overview_tests.rs",
        "overview_six_item_metrics_grid_mounts_and_updates_in_place",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/overview_stats.rs",
        "overview_connections",
        "overview_memory",
        "overview_upload",
        "overview_download",
    )
    require(
        violations,
        "scripts/test.sh",
        "overview-metrics-grid-guard.py --mode enforce",
    )
    require(
        violations,
        "scripts/test-bevy.sh",
        "overview-metrics-grid-guard.py\" --mode enforce",
    )

    if violations:
        for violation in violations:
            print(f"overview-metrics-grid-guard: {violation}", file=sys.stderr)
        print(
            f"overview-metrics-grid-guard: violations={len(violations)}",
            file=sys.stderr,
        )
        return 1 if args.mode == "enforce" else 0
    print("overview-metrics-grid-guard: DUAL-03-10 markers=complete violations=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
