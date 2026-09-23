#!/usr/bin/env python3
"""Fail-closed guard for DUAL-06-04 / DUAL-06-11 speedtest closure.

DUAL-06-04 (real downlink bandwidth) must be a seam-level, honest path: the
host reports measured bytes/duration through the shared port and engine, never
a fabricated UI number. DUAL-06-11 (history persistence) must round-trip the
canonical `recent_history` through a host store so both surfaces read one
shared snapshot across restarts — no UI-local history source.
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
            violations.append(f"{path} still contains forbidden path {marker!r}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--mode", choices=["report", "enforce"], default="enforce")
    args = parser.parse_args()
    violations: list[str] = []

    require(
        violations,
        "docs/DUAL_SURFACE_PARITY_MASTER_PLAN.md",
        "DUAL-06-04",
        "DUAL-06-11",
        "parity-ready",
    )
    # DUAL-06-04: the bandwidth seam is contract/port/application/host.
    require(
        violations,
        "crates/infiltrator-contract/src/command.rs",
        "RecordSpeedtestBandwidth",
    )
    require(
        violations,
        "crates/infiltrator-ports/src/speedtest.rs",
        "fn record_bandwidth",
    )
    require(
        violations,
        "crates/infiltrator-application/src/speedtest_application.rs",
        "fn record_bandwidth",
        "with_history_store",
        "SpeedtestHistoryStore",
        "persist_history",
    )
    require(
        violations,
        "crates/infiltrator-application/src/command_application.rs",
        "RecordSpeedtestBandwidth",
        "record_bandwidth",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/speedtest.rs",
        "fn record_bandwidth",
    )
    # DUAL-06-11: one durable store port, hydrated on construction.
    require(
        violations,
        "crates/infiltrator-ports/src/speedtest_history.rs",
        "trait SpeedtestHistoryStore",
        "fn load",
        "fn save",
    )
    require(violations, "crates/infiltrator-ports/src/lib.rs", "speedtest_history")
    require(
        violations,
        "crates/infiltrator-desktop/src/speedtest_history_store.rs",
        "SpeedtestHistoryStore",
        "fn load",
        "fn save",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/runtime.rs",
        "with_history_store",
    )
    # Both surfaces render the shared snapshot's `recent_history`.
    require(
        violations,
        "crates/infiltrator-iced/src/view/speedtest_modal.rs",
        "snapshot.recent_history",
        "shared_speedtest_history_lines",
        "speedtest_history_title",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/overview_speedtest.rs",
        "OverviewSpeedtestHistoryText",
        "recent_history",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/iced_six_advancements_wave3_tests.rs",
        "test_advancement_w3_3_speedtest_history_renders_shared_snapshot",
        "shared_speedtest_history_lines",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/overview_tests.rs",
        "overview_speedtest_history_follows_shared_engine",
        "OverviewSpeedtestHistoryText",
    )
    # Neither inbound surface may own a persistence source of its own.
    forbid(
        violations,
        "crates/infiltrator-iced/src/view/speedtest_modal.rs",
        "SpeedtestHistoryStore",
        "speedtest_history.json",
    )
    forbid(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/overview_speedtest.rs",
        "SpeedtestHistoryStore",
        "speedtest_history.json",
    )
    require(
        violations,
        "scripts/test.sh",
        "speedtest-history-guard.py --mode enforce",
    )
    require(
        violations,
        "scripts/test-bevy.sh",
        'speedtest-history-guard.py" --mode enforce',
    )

    if violations:
        for violation in violations:
            print(f"speedtest-history-guard: {violation}", file=sys.stderr)
        print(
            f"speedtest-history-guard: violations={len(violations)}",
            file=sys.stderr,
        )
        return 1 if args.mode == "enforce" else 0
    print("speedtest-history-guard: DUAL-06-04/11 markers=complete violations=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
