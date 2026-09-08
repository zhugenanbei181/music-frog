#!/usr/bin/env python3
"""Fail-closed guard for DUAL-04-05 Proxy Four-way Sorting parity."""

from __future__ import annotations

import argparse
import pathlib
import sys


ROOT = pathlib.Path(__file__).resolve().parents[2]


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def require(violations: list[str], path: str, *markers: str) -> None:
    text = read(path)
    compact_text = " ".join(text.split())
    for marker in markers:
        compact_marker = " ".join(marker.split())
        rustfmt_marker = compact_marker.replace(" }", ", }")
        if marker not in text and compact_marker not in compact_text and rustfmt_marker not in compact_text:
            violations.append(f"{path} missing {marker!r}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--mode", choices=["report", "enforce"], default="enforce")
    args = parser.parse_args()
    violations: list[str] = []

    require(
        violations,
        "docs/DUAL_SURFACE_PARITY_MASTER_PLAN.md",
        "DUAL-04-05",
        "四维排序控制器",
        "ProxySortOrder",
        "parity-ready",
    )
    require(
        violations,
        "docs/CORE_030_REARCHITECTURE.md",
        "DUAL-04-05",
        "ProxySortOrder",
        "SetProxySortOrder",
    )
    require(
        violations,
        "docs/DUAL_SURFACE_ARCHITECTURE_AUDIT_030.md",
        "DUAL-04-05",
        "ProxySortPill",
        "host-verified",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/proxies.rs",
        "pub enum ProxySortOrder",
        "LatencyAsc",
        "LatencyDesc",
        "NameAsc",
        "NameDesc",
        "compare_candidates",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/command.rs",
        "SetProxySortOrder { order: crate::proxies::ProxySortOrder }",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/proxies.rs",
        "ProxySortPill",
        "ProxySortMode",
        "SetProxySortOrder",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/pages_matrix_a_tests.rs",
        "test_proxies_sort_pills_submit_commands",
        "ProxySortPill",
    )
    require(
        violations,
        "scripts/test.sh",
        "proxies-sorting-guard.py --mode enforce",
    )
    require(
        violations,
        "scripts/test-bevy.sh",
        "proxies-sorting-guard.py\" --mode enforce",
    )

    if violations:
        for violation in violations:
            print(f"proxies-sorting-guard: {violation}", file=sys.stderr)
        print(
            f"proxies-sorting-guard: violations={len(violations)}",
            file=sys.stderr,
        )
        return 1 if args.mode == "enforce" else 0
    print("proxies-sorting-guard: DUAL-04-05 markers=complete violations=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
