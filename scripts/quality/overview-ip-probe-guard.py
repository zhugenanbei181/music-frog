#!/usr/bin/env python3
"""Fail-closed guard for DUAL-03-11 Overview public IP probe card parity."""

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
        "DUAL-03-11",
        "公网 IP 隐私归属探针",
        "parity-ready",
    )
    require(
        violations,
        "docs/CORE_030_REARCHITECTURE.md",
        "DUAL-03-11",
        "PublicIpProbeSnapshot",
        "RefreshPublicIpProbe",
    )
    require(
        violations,
        "docs/DUAL_SURFACE_ARCHITECTURE_AUDIT_030.md",
        "DUAL-03-11",
        "PublicIpProbeCard",
        "host-verified",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/public_ip.rs",
        "PublicIpProbeSnapshot",
        "PublicIpProbeStatus",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/overview_public_ip.rs",
        "PublicIpProbeCard",
        "PublicIpRefreshButton",
        "on_overview_public_ip_refresh_activated",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/overview_tests.rs",
        "overview_public_ip_probe_card_mounts_and_updates_in_place",
        "overview_public_ip_refresh_button_submits_refresh_command",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/overview_ip.rs",
        "current_ip_card",
        "overview_current_ip",
    )
    require(
        violations,
        "scripts/test.sh",
        "overview-ip-probe-guard.py --mode enforce",
    )
    require(
        violations,
        "scripts/test-bevy.sh",
        "overview-ip-probe-guard.py\" --mode enforce",
    )

    if violations:
        for violation in violations:
            print(f"overview-ip-probe-guard: {violation}", file=sys.stderr)
        print(
            f"overview-ip-probe-guard: violations={len(violations)}",
            file=sys.stderr,
        )
        return 1 if args.mode == "enforce" else 0
    print("overview-ip-probe-guard: DUAL-03-11 markers=complete violations=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
