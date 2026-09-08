#!/usr/bin/env python3
"""Fail-closed guard for DUAL-04-10 Proxy Pinyin & Protocol Fuzzy Filter parity."""

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
        "DUAL-04-10",
        "智能拼音与协议模糊检索",
        "matches_proxy_filter",
        "parity-ready",
    )
    require(
        violations,
        "docs/CORE_030_REARCHITECTURE.md",
        "DUAL-04-10",
        "matches_proxy_filter",
    )
    require(
        violations,
        "docs/DUAL_SURFACE_ARCHITECTURE_AUDIT_030.md",
        "DUAL-04-10",
        "matches_proxy_filter",
        "host-verified",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/proxies_filter.rs",
        "pub fn matches_proxy_filter",
        "node_flag",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/pages_matrix_a_tests.rs",
        "test_proxies_pinyin_fuzzy_and_protocol_filtering",
        "matches_proxy_filter",
    )
    require(
        violations,
        "scripts/test.sh",
        "proxies-fuzzy-filter-guard.py --mode enforce",
    )
    require(
        violations,
        "scripts/test-bevy.sh",
        "proxies-fuzzy-filter-guard.py\" --mode enforce",
    )

    if violations:
        for violation in violations:
            print(f"proxies-fuzzy-filter-guard: {violation}", file=sys.stderr)
        print(
            f"proxies-fuzzy-filter-guard: violations={len(violations)}",
            file=sys.stderr,
        )
        return 1 if args.mode == "enforce" else 0
    print("proxies-fuzzy-filter-guard: DUAL-04-10 markers=complete violations=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
