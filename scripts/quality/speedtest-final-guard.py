#!/usr/bin/env python3
"""Fail-closed guard for DUAL-06-12/13/14 (speedtest group closure).

Both surfaces must read the one shared `SpeedtestApplication` snapshot: the
egress-IP comparison, the per-node detail modal and the dual-surface state
machine matrix are all driven by that single read model. No UI-local facts.
"""

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
        if (
            marker not in text
            and compact_marker not in compact_text
            and rustfmt_marker not in compact_text
        ):
            violations.append(f"{path} missing {marker!r}")


def forbid(violations: list[str], path: str, *markers: str) -> None:
    text = read(path)
    for marker in markers:
        if marker in text:
            violations.append(f"{path} still contains legacy path {marker!r}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--mode", choices=["report", "enforce"], default="enforce")
    args = parser.parse_args()
    violations: list[str] = []

    # 06-12: egress IP seam + honest label/egress comparison.
    require(
        violations,
        "crates/infiltrator-contract/src/speedtest.rs",
        "EgressCountryMatch",
        "label_country",
        "egress_country_match",
        "egress_endpoint_label",
        "egress_summary",
        "egress_country_mismatches",
    )
    require(
        violations,
        "crates/infiltrator-ports/src/speedtest.rs",
        "fn record_outbound_ip",
        "async fn probe_outbound_ip",
    )
    require(
        violations,
        "crates/infiltrator-application/src/speedtest_application.rs",
        "pub fn record_outbound_ip",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/speedtest.rs",
        "fn record_outbound_ip",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/command.rs",
        "RecordSpeedtestOutboundIp",
    )
    require(
        violations,
        "crates/infiltrator-application/src/command_application.rs",
        "CommandIntent::RecordSpeedtestOutboundIp",
        "record_outbound_ip",
    )
    require(
        violations,
        "crates/infiltrator-application/src/core_application.rs",
        "record_speedtest_outbound_ip",
    )

    # 06-13: per-node detail modal on both surfaces.
    require(
        violations,
        "crates/infiltrator-iced/src/types/message.rs",
        "OpenSpeedtestDetail",
        "CloseSpeedtestDetail",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/speedtest_modal.rs",
        "Message::OpenSpeedtestDetail",
        "egress_endpoint_label",
        "egress_country_match",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view_root/speedtest_detail_modal.rs",
        "speedtest_detail_modal",
        "egress_summary",
        "speedtest_detail_empty",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view_root.rs",
        "speedtest_detail_modal",
        "speedtest_detail_open",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/overview.rs",
        "OverviewSpeedtestEgressText",
        "OverviewSpeedtestDetailButton",
        "OverviewSpeedtestDetailBodyText",
        "overview_speedtest_detail_modal_scene",
        "sync_overview_speedtest_detail",
        "OpenModal",
        "egress_endpoint_label",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/shell_scene.rs",
        "overview_speedtest_detail_modal_scene",
    )

    # 06-14: shared dual-surface regression matrix.
    require(
        violations,
        "crates/infiltrator-contract/src/speedtest_matrix.rs",
        "SpeedtestRegressionMatrixReport",
        "run_deterministic_matrix",
        "dual_surface_check",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/lib.rs",
        "pub mod speedtest_matrix;",
    )
    require(
        violations,
        "crates/infiltrator-application/src/speedtest_matrix_application.rs",
        "SpeedtestMatrixApplication",
        "verify_dual_surface_snapshot",
    )
    require(
        violations,
        "crates/infiltrator-application/src/lib.rs",
        "pub mod speedtest_matrix_application;",
    )

    # Dual headless tests.
    require(
        violations,
        "crates/infiltrator-application/src/speedtest_application_tests.rs",
        "test_record_outbound_ip_populates_and_compares_label_country",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/iced_six_advancements_wave3_tests.rs",
        "test_advancement_w3_3_speedtest_egress_detail_and_matrix",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/overview_tests.rs",
        "overview_speedtest_egress_and_detail_modal_follow_shared_engine",
    )

    # Ledger + guard registration.
    require(
        violations,
        "docs/DUAL_SURFACE_PARITY_MASTER_PLAN.md",
        "DUAL-06-12",
        "DUAL-06-13",
        "DUAL-06-14",
        "parity-ready",
    )
    require(
        violations,
        "scripts/test.sh",
        "speedtest-final-guard.py --mode enforce",
    )
    require(
        violations,
        "scripts/test-bevy.sh",
        'speedtest-final-guard.py" --mode enforce',
    )

    # Neither surface may fabricate egress facts locally.
    forbid(
        violations,
        "crates/infiltrator-iced/src/view/speedtest_modal.rs",
        "outbound_ip: Some(",
        "outbound_country: Some(",
    )

    if violations:
        for violation in violations:
            print(f"speedtest-final-guard: {violation}", file=sys.stderr)
        print(
            f"speedtest-final-guard: violations={len(violations)}",
            file=sys.stderr,
        )
        return 1 if args.mode == "enforce" else 0
    print("speedtest-final-guard: DUAL-06-12/13/14 markers=complete violations=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
