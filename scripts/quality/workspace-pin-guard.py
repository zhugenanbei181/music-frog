#!/usr/bin/env python3
"""Dynamic workspace-invariant guard.

Validates structural workspace facts by *parsing manifests*, never by matching
literal version strings in a workflow:

1. The Bevy product crates are workspace members.
2. The shared `bevy` dependency is pinned to an exact version (`=x.y.z`).
3. `infiltrator-iced` carries the renamed naga coexistence workaround.

This is the single registration point for these invariants; CI workflows must
not inline their own literal copies.
"""

from __future__ import annotations

import argparse
import pathlib
import sys
import tomllib

ROOT = pathlib.Path(__file__).resolve().parents[2]
BEVY_MEMBERS = ("crates/infiltrator-bevy-widgets", "crates/infiltrator-bevy-ui")
DEP_SECTIONS = ("dependencies", "dev-dependencies", "build-dependencies")


def load(rel: str) -> dict:
    return tomllib.loads((ROOT / rel).read_text(encoding="utf-8"))


def collect_violations() -> list[str]:
    violations: list[str] = []

    root = load("Cargo.toml")
    members = root.get("workspace", {}).get("members", [])
    for member in BEVY_MEMBERS:
        if member not in members:
            violations.append(f"Cargo.toml [workspace] members missing {member}")

    bevy = root.get("workspace", {}).get("dependencies", {}).get("bevy")
    version = bevy.get("version") if isinstance(bevy, dict) else None
    if not (isinstance(version, str) and version.startswith("=")):
        violations.append(
            f"workspace `bevy` dependency must be pinned to an exact version "
            f"(=x.y.z), found {version!r}"
        )

    iced = load("crates/infiltrator-iced/Cargo.toml")
    deps: dict = {}
    for section in DEP_SECTIONS:
        deps.update(iced.get(section, {}) or {})
    has_naga_workaround = any(
        isinstance(spec, dict) and spec.get("package") == "naga" for spec in deps.values()
    )
    if not has_naga_workaround:
        violations.append(
            "crates/infiltrator-iced/Cargo.toml must declare the renamed naga "
            'coexistence workaround (`package = "naga"`)'
        )
    return violations


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=("report", "enforce"), default="report")
    args = parser.parse_args()

    violations = collect_violations()
    if not violations:
        print("workspace-pin-guard: OK")
        return 0
    for violation in violations:
        print(f"workspace-pin-guard: {violation}", file=sys.stderr)
    print(f"workspace-pin-guard: violations={len(violations)}", file=sys.stderr)
    return 1 if args.mode == "enforce" else 0


if __name__ == "__main__":
    sys.exit(main())
