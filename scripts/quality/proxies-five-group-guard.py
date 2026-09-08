#!/usr/bin/env python3
"""Fail-closed guard for DUAL-04-01 Five Canonical Proxy Group Classifications parity."""

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
        "DUAL-04-01",
        "策略组 5 大分类全覆盖",
        "ProxyGroupClassification",
        "parity-ready",
    )
    require(
        violations,
        "docs/CORE_030_REARCHITECTURE.md",
        "DUAL-04-01",
        "ProxyGroupClassification",
        "Selector",
        "UrlTest",
        "Fallback",
        "LoadBalance",
        "Relay",
    )
    require(
        violations,
        "docs/DUAL_SURFACE_ARCHITECTURE_AUDIT_030.md",
        "DUAL-04-01",
        "ProxyGroupClassification",
        "host-verified",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/proxies.rs",
        "pub enum ProxyGroupClassification",
        "Selector",
        "UrlTest",
        "Fallback",
        "LoadBalance",
        "Relay",
        "is_manual_selectable",
    )
    require(
        violations,
        "crates/infiltrator-application/src/proxy_application.rs",
        "ProxyGroupClassification",
        "list_group_details",
        "is_manual_selectable",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/proxies.rs",
        "ProxyGroup",
        "classification",
    )
    require(
        violations,
        "scripts/test.sh",
        "proxies-five-group-guard.py --mode enforce",
    )
    require(
        violations,
        "scripts/test-bevy.sh",
        "proxies-five-group-guard.py\" --mode enforce",
    )

    if violations:
        for violation in violations:
            print(f"proxies-five-group-guard: {violation}", file=sys.stderr)
        print(
            f"proxies-five-group-guard: violations={len(violations)}",
            file=sys.stderr,
        )
        return 1 if args.mode == "enforce" else 0
    print("proxies-five-group-guard: DUAL-04-01 markers=complete violations=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
