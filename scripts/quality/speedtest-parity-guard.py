#!/usr/bin/env python3
"""Fail-closed guard for DUAL-06-02/05/06/07/08/10 speedtest dual-surface parity.

Both surfaces must read the one shared `SpeedtestApplication` snapshot: no
legacy proxy-delay path on Iced, no Bevy-local metrics, and cancel/progress
driven from the same engine.
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

    require(
        violations,
        "docs/DUAL_SURFACE_PARITY_MASTER_PLAN.md",
        "DUAL-06-02",
        "DUAL-06-05",
        "DUAL-06-06",
        "DUAL-06-07",
        "DUAL-06-08",
        "DUAL-06-10",
        "parity-ready",
    )
    require(
        violations,
        "crates/infiltrator-ports/src/speedtest.rs",
        "run_scope",
        "fn cancel",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update/core/proxies.rs",
        "run_speedtest_scope",
        "SpeedtestScopeUpdated",
        "CancelSpeedtest",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/types/message.rs",
        "SpeedtestScopeUpdated",
        "CancelSpeedtest",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/overview.rs",
        "speedtest_cancel",
        "Message::CancelSpeedtest",
        "snapshot.is_running()",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/overview.rs",
        "OverviewSpeedtestMetricsText",
        "UiCommand::CancelSpeedtest",
        "fastest_node()",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/command.rs",
        "CancelSpeedtest",
        "CommandIntent::CancelSpeedtest",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/iced_six_advancements_wave3_tests.rs",
        "test_advancement_w3_3_speedtest_scope_and_cancel_share_the_engine",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/overview_tests.rs",
        "overview_speedtest_metrics_follow_shared_engine",
        "overview_speedtest_running_button_submits_cancel",
    )
    # Iced must not regress to the legacy proxy-delay path.
    forbid(violations, "crates/infiltrator-iced/src/update/core/proxies.rs", "test_proxy_delays")
    require(
        violations,
        "scripts/test.sh",
        "speedtest-parity-guard.py --mode enforce",
    )
    require(
        violations,
        "scripts/test-bevy.sh",
        'speedtest-parity-guard.py" --mode enforce',
    )

    if violations:
        for violation in violations:
            print(f"speedtest-parity-guard: {violation}", file=sys.stderr)
        print(
            f"speedtest-parity-guard: violations={len(violations)}",
            file=sys.stderr,
        )
        return 1 if args.mode == "enforce" else 0
    print("speedtest-parity-guard: DUAL-06-02/05/06/07/08/10 markers=complete violations=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
