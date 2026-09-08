#!/usr/bin/env python3
"""Fail-closed guard for DUAL-04-07 Proxy Protocol & Feature Chips parity."""

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
        "DUAL-04-07",
        "协议与特性高级芯片",
        "format_protocol_chip",
        "parity-ready",
    )
    require(
        violations,
        "docs/CORE_030_REARCHITECTURE.md",
        "DUAL-04-07",
        "format_protocol_chip",
        "NodeProtoText",
    )
    require(
        violations,
        "docs/DUAL_SURFACE_ARCHITECTURE_AUDIT_030.md",
        "DUAL-04-07",
        "NodeProtoText",
        "NodeUdpTag",
        "host-verified",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/proxies_filter.rs",
        "pub fn format_protocol_chip",
        "Shadowsocks",
        "Vless",
        "VMess",
        "Hysteria2",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/proxies.rs",
        "NodeProtoText",
        "NodeUdpTag",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/pages_matrix_a_tests.rs",
        "test_proxies_advanced_chips_and_color_ladder",
        "format_protocol_chip",
    )
    require(
        violations,
        "scripts/test.sh",
        "proxies-protocol-chips-guard.py --mode enforce",
    )
    require(
        violations,
        "scripts/test-bevy.sh",
        "proxies-protocol-chips-guard.py\" --mode enforce",
    )

    if violations:
        for violation in violations:
            print(f"proxies-protocol-chips-guard: {violation}", file=sys.stderr)
        print(
            f"proxies-protocol-chips-guard: violations={len(violations)}",
            file=sys.stderr,
        )
        return 1 if args.mode == "enforce" else 0
    print("proxies-protocol-chips-guard: DUAL-04-07 markers=complete violations=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
