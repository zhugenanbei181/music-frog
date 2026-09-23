#!/usr/bin/env python3
"""Fail-closed guard for DUAL-03-04 topology drill-down navigation parity."""

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
        "DUAL-03-04",
        "拓扑节点下钻跳转交互",
        "TrafficTopologyNavigationTarget",
        "parity-ready",
    )
    require(
        violations,
        "docs/CORE_030_REARCHITECTURE.md",
        "DUAL-03-04",
        "TrafficTopologyNavigationApplication",
        "RouteChanged",
        "Elm Navigate",
    )
    require(
        violations,
        "docs/DUAL_SURFACE_ARCHITECTURE_AUDIT_030.md",
        "DUAL-03-04",
        "TrafficTopologyNavigationTarget/TrafficTopologyNavigationApplication",
        "host-verified",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/traffic_topology.rs",
        "TrafficTopologyNavigationTarget",
        "Settings",
        "Rules",
        "Proxies",
    )
    require(
        violations,
        "crates/infiltrator-application/src/traffic_topology_navigation_application.rs",
        "TrafficTopologyNavigationApplication",
        "target_for_stage",
        "page_for_stage",
        "every_topology_stage_has_one_shared_destination",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/overview_topology.rs",
        "TopologyStageButton",
        "on_topology_stage_activated",
        "TrafficTopologyNavigationApplication::page_for_stage",
        "RouteChanged",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/overview_topology.rs",
        "TopologyStageButton { stage",
        "enabled: { snapshot.is_drawable() }",
        "Button",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/overview_tests.rs",
        "topology_stage_activation_uses_shared_navigation_targets",
        "Activate { entity: button }",
        "!button.enabled",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/overview.rs",
        "TrafficTopologyNavigationApplication::page_for_stage",
        "Message::Navigate(route)",
        "topology_route_for_stage",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/overview_tests.rs",
        "topology_stage_routes_follow_the_shared_application_mapping",
    )
    require(
        violations,
        "scripts/test.sh",
        "traffic-topology-navigation-guard.py --mode enforce",
    )
    require(
        violations,
        "scripts/test-bevy.sh",
        "traffic-topology-navigation-guard.py\" --mode enforce",
    )

    if violations:
        for violation in violations:
            print(f"traffic-topology-navigation-guard: {violation}", file=sys.stderr)
        print(
            f"traffic-topology-navigation-guard: violations={len(violations)}",
            file=sys.stderr,
        )
        return 1 if args.mode == "enforce" else 0
    print("traffic-topology-navigation-guard: DUAL-03-04 markers=complete violations=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
