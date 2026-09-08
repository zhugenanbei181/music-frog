#!/usr/bin/env python3
"""Fail-closed guard for DUAL-03-15 Overview headless regression matrix parity."""

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
        "DUAL-03-15",
        "概览双端全景无头行为与回归测试矩阵",
        "OverviewRegressionMatrixReport",
        "parity-ready",
    )
    require(
        violations,
        "docs/CORE_030_REARCHITECTURE.md",
        "DUAL-03-15",
        "OverviewRegressionMatrixReport",
        "run_deterministic_matrix",
    )
    require(
        violations,
        "docs/DUAL_SURFACE_ARCHITECTURE_AUDIT_030.md",
        "DUAL-03-15",
        "OverviewRegressionMatrixReport",
        "host-verified",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/overview_matrix.rs",
        "OverviewRegressionMatrixReport",
        "OverviewRegressionScenario",
        "run_deterministic_matrix",
    )
    require(
        violations,
        "crates/infiltrator-application/src/overview_matrix_application.rs",
        "OverviewMatrixApplication",
        "execute",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/overview_tests.rs",
        "test_overview_dual_surface_headless_regression_matrix_full_coverage",
        "OverviewRegressionMatrixReport",
    )
    require(
        violations,
        "scripts/test.sh",
        "overview-regression-matrix-guard.py --mode enforce",
    )
    require(
        violations,
        "scripts/test-bevy.sh",
        "overview-regression-matrix-guard.py\" --mode enforce",
    )

    if violations:
        for violation in violations:
            print(f"overview-regression-matrix-guard: {violation}", file=sys.stderr)
        print(
            f"overview-regression-matrix-guard: violations={len(violations)}",
            file=sys.stderr,
        )
        return 1 if args.mode == "enforce" else 0
    print("overview-regression-matrix-guard: DUAL-03-15 markers=complete violations=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
