#!/usr/bin/env python3
"""Fail-closed guard for DUAL-04-04 Proxy Filter Alive parity."""

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
        "DUAL-04-04",
        "节点死链一键隐藏",
        "ProxyFilterAliveSnapshot",
        "parity-ready",
    )
    require(
        violations,
        "docs/CORE_030_REARCHITECTURE.md",
        "DUAL-04-04",
        "ProxyFilterAliveSnapshot",
        "ToggleFilterAlive",
    )
    require(
        violations,
        "docs/DUAL_SURFACE_ARCHITECTURE_AUDIT_030.md",
        "DUAL-04-04",
        "FilterAliveToggle",
        "host-verified",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/proxies.rs",
        "ProxyFilterAliveSnapshot",
        "is_node_alive",
        "derive_from_candidates",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/command.rs",
        "ToggleFilterAlive { enabled: bool }",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/proxies.rs",
        "FilterAliveToggle",
        "ToggleFilterAlive",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/pages_matrix_a_tests.rs",
        "test_proxies_filter_alive_toggle_submits_command",
        "FilterAliveToggle",
    )
    require(
        violations,
        "scripts/test.sh",
        "proxies-filter-alive-guard.py --mode enforce",
    )
    require(
        violations,
        "scripts/test-bevy.sh",
        "proxies-filter-alive-guard.py\" --mode enforce",
    )

    if violations:
        for violation in violations:
            print(f"proxies-filter-alive-guard: {violation}", file=sys.stderr)
        print(
            f"proxies-filter-alive-guard: violations={len(violations)}",
            file=sys.stderr,
        )
        return 1 if args.mode == "enforce" else 0
    print("proxies-filter-alive-guard: DUAL-04-04 markers=complete violations=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
