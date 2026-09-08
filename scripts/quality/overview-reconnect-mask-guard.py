#!/usr/bin/env python3
"""Fail-closed guard for DUAL-03-13 Overview reload/reconnect graceful degradation mask parity."""

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
        "DUAL-03-13",
        "断线与重载优雅降级蒙版",
        "ReconnectMaskSnapshot",
        "parity-ready",
    )
    require(
        violations,
        "docs/CORE_030_REARCHITECTURE.md",
        "DUAL-03-13",
        "ReconnectMaskSnapshot",
        "preserves_last_frame",
    )
    require(
        violations,
        "docs/DUAL_SURFACE_ARCHITECTURE_AUDIT_030.md",
        "DUAL-03-13",
        "OverviewReloadMask",
        "host-verified",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/reconnect_mask.rs",
        "ReconnectMaskSnapshot",
        "ReconnectMaskStatus",
    )
    require(
        violations,
        "crates/infiltrator-application/src/reconnect_mask_application.rs",
        "ReconnectMaskApplication",
        "project",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/overview.rs",
        "OverviewReloadMask",
        "OverviewReloadMaskText",
        "reload_mask_scene",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/overview_tests.rs",
        "overview_reload_mask_activates_and_preserves_facts",
        "OverviewReloadMask",
    )
    require(
        violations,
        "scripts/test.sh",
        "overview-reconnect-mask-guard.py --mode enforce",
    )
    require(
        violations,
        "scripts/test-bevy.sh",
        "overview-reconnect-mask-guard.py\" --mode enforce",
    )

    if violations:
        for violation in violations:
            print(f"overview-reconnect-mask-guard: {violation}", file=sys.stderr)
        print(
            f"overview-reconnect-mask-guard: violations={len(violations)}",
            file=sys.stderr,
        )
        return 1 if args.mode == "enforce" else 0
    print("overview-reconnect-mask-guard: DUAL-03-13 markers=complete violations=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
