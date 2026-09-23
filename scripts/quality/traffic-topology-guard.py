#!/usr/bin/env python3
"""Fail-closed guard for DUAL-03-03 live traffic topology parity."""

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
        "DUAL-03-03",
        "分流链路可视化拓扑流动链",
        "TrafficTopologySnapshot",
        "parity-ready",
    )
    require(
        violations,
        "docs/CORE_030_REARCHITECTURE.md",
        "DUAL-03-03",
        "TrafficTopologyApplication",
        "Inbound",
        "Proxy Group",
        "Outbound",
    )
    require(
        violations,
        "docs/DUAL_SURFACE_ARCHITECTURE_AUDIT_030.md",
        "DUAL-03-03",
        "TrafficTopologySnapshot/TrafficTopologyApplication",
        "host-verified",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/traffic_topology.rs",
        "TRAFFIC_TOPOLOGY_STAGE_COUNT",
        "TrafficTopologyStage",
        "TrafficTopologyStatus",
        "TrafficTopologySnapshot",
        "demo_fixture",
        "is_flowing",
    )
    require(
        violations,
        "crates/infiltrator-domain/src/traffic_topology.rs",
        "TrafficTopologyInput",
        "pub fn derive",
        "summarize_rules",
        "select_group",
        "select_outbound",
        "derives_five_stage_live_chain_from_connections_and_config",
        "empty_connections_keep_real_config_but_disable_flow",
    )
    require(
        violations,
        "crates/infiltrator-application/src/traffic_topology_application.rs",
        "TrafficTopologyApplication",
        "pub fn project",
        "missing_gateway_is_typed_unsupported",
        "stopped_core_does_not_reuse_traffic_as_a_live_topology",
    )
    require(
        violations,
        "crates/infiltrator-application/src/surface_reader.rs",
        "traffic_topology",
        "TrafficTopologyApplication",
        "self.traffic_topology.project",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/surface_snapshot.rs",
        "pub traffic_topology: crate::traffic_topology::TrafficTopologySnapshot",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/overview_topology.rs",
        "topology_chain_scene_with_snapshot",
        "topology_spec",
        "topology_scene",
        "TopologyText",
        "TopologyArrow",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/overview_restamp.rs",
        "projection.traffic_topology",
        "topology_text_value",
        "TopologyPlate",
    )
    require(
        violations,
        "crates/infiltrator-bevy-widgets/src/chart/topology.rs",
        "flow_phase",
        "flow_speed",
        "draw_particle",
        "advance_topology_flow",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/state.rs",
        "pub traffic_topology",
        "snapshot.traffic_topology",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/overview_topology.rs",
        "state.runtime.traffic_topology",
        "topology_flow_canvas",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/overview_tests.rs",
        "topology_badge_reflects_the_shared_status_and_count",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/topology.rs",
        "TopologyFlowCanvas",
        "is_flowing",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/topology_tests.rs",
        "flow_strip_keeps_phase_bounded_and_uses_shared_snapshot",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/surface_contract_tests.rs",
        "shared_topology_snapshot_reaches_the_iced_runtime_projection",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/overview_tests.rs",
        "test_topology_chain_card_mounts_with_five_stages_and_flow_plate",
        "topology_projection_updates_text_and_flow_plate_in_place",
        "TopologyPlate",
    )
    require(violations, "scripts/test.sh", "traffic-topology-guard.py --mode enforce")
    require(
        violations,
        "scripts/test-bevy.sh",
        "traffic-topology-guard.py\" --mode enforce",
    )

    if violations:
        for violation in violations:
            print(f"traffic-topology-guard: {violation}", file=sys.stderr)
        print(f"traffic-topology-guard: violations={len(violations)}", file=sys.stderr)
        return 1 if args.mode == "enforce" else 0
    print("traffic-topology-guard: DUAL-03-03 markers=complete violations=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
