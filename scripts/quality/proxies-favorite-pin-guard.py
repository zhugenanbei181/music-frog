#!/usr/bin/env python3
"""Fail-closed guard for DUAL-04-06 Proxy Favorite Pinning parity."""

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
        "DUAL-04-06",
        "节点星标置顶与收藏",
        "favorite_proxies",
        "parity-ready",
    )
    require(
        violations,
        "docs/CORE_030_REARCHITECTURE.md",
        "DUAL-04-06",
        "favorite_proxies",
        "ToggleFavoriteProxy",
    )
    require(
        violations,
        "docs/DUAL_SURFACE_ARCHITECTURE_AUDIT_030.md",
        "DUAL-04-06",
        "NodePinButton",
        "host-verified",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/proxies.rs",
        "pub favorite_proxies: Vec<String>",
        "is_favorite",
        "toggle_favorite",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/command.rs",
        "ToggleFavoriteProxy { proxy: String }",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/proxies.rs",
        "NodePinButton",
        "ToggleFavoriteProxy",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/pages_matrix_a_tests.rs",
        "test_proxies_favorite_pin_submits_command",
        "NodePinButton",
    )
    require(
        violations,
        "scripts/test.sh",
        "proxies-favorite-pin-guard.py --mode enforce",
    )
    require(
        violations,
        "scripts/test-bevy.sh",
        "proxies-favorite-pin-guard.py\" --mode enforce",
    )

    if violations:
        for violation in violations:
            print(f"proxies-favorite-pin-guard: {violation}", file=sys.stderr)
        print(
            f"proxies-favorite-pin-guard: violations={len(violations)}",
            file=sys.stderr,
        )
        return 1 if args.mode == "enforce" else 0
    print("proxies-favorite-pin-guard: DUAL-04-06 markers=complete violations=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
