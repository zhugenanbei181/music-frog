#!/usr/bin/env python3
"""Fail-closed guard for DUAL-03-05 active-exit card parity."""

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
        "DUAL-03-05",
        "主活动出口节点高保真卡片",
        "ActiveExitSnapshot",
        "parity-ready",
    )
    require(
        violations,
        "docs/CORE_030_REARCHITECTURE.md",
        "DUAL-03-05",
        "ActiveExitApplication",
        "selected node",
        "Empty/Unsupported/Failed",
    )
    require(
        violations,
        "docs/DUAL_SURFACE_ARCHITECTURE_AUDIT_030.md",
        "DUAL-03-05",
        "ActiveExitSnapshot/ActiveExitApplication",
        "host-verified",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/active_exit.rs",
        "ActiveExitStatus",
        "ActiveExitSnapshot",
        "country_code",
        "protocol",
        "delay_ms",
        "alive",
        "demo_fixture",
    )
    require(
        violations,
        "crates/infiltrator-domain/src/active_exit.rs",
        "pub fn derive",
        "selected_group",
        "extract_country_code",
        "derives_selected_group_node_protocol_delay_and_region",
        "empty_proxy_map_is_typed_empty",
    )
    require(
        violations,
        "crates/infiltrator-application/src/active_exit_application.rs",
        "ActiveExitApplication",
        "pub fn project",
        "missing_proxy_port_is_typed_unsupported",
        "stopped_core_does_not_claim_a_selected_exit",
    )
    require(
        violations,
        "crates/infiltrator-application/src/surface_reader.rs",
        "active_exit_snapshot",
        "self.active_exit.project",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/surface_snapshot.rs",
        "pub active_exit: crate::active_exit::ActiveExitSnapshot",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/overview_cards.rs",
        "active_exit_node_scene_with_snapshot",
        "ActiveExitText",
        "active_exit_text_value",
        "ActiveExitSnapshot::demo_fixture",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/overview_restamp.rs",
        "pub struct ActiveExitText",
        "projection.active_exit",
        "active_exit_text_value",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/overview_tests.rs",
        "test_overview_master_switches_and_exit_node_cards",
        "active_exit_projection_restsamps_facts_and_failure_in_place",
        "ActiveExitNodeCard",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/state.rs",
        "pub active_exit",
        "snapshot.active_exit",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/active_exit.rs",
        "active_exit_card",
        "country_flag",
        "status_text",
        "protocol",
        "delay",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/active_exit_tests.rs",
        "status_text_tracks_shared_liveness_without_guessing",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/overview.rs",
        "active_exit_card(state, &lang)",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/demo_tests.rs",
        "traffic_topology.nodes.len(), 5",
        "traffic_topology.is_flowing",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/surface_contract_tests.rs",
        "shared_topology_snapshot_reaches_the_iced_runtime_projection",
    )
    require(violations, "scripts/test.sh", "active-exit-guard.py --mode enforce")
    require(
        violations,
        "scripts/test-bevy.sh",
        "active-exit-guard.py\" --mode enforce",
    )

    if violations:
        for violation in violations:
            print(f"active-exit-guard: {violation}", file=sys.stderr)
        print(f"active-exit-guard: violations={len(violations)}", file=sys.stderr)
        return 1 if args.mode == "enforce" else 0
    print("active-exit-guard: DUAL-03-05 markers=complete violations=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
