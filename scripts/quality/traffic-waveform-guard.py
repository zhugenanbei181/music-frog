#!/usr/bin/env python3
"""Fail-closed guard for DUAL-03-01 live traffic waveform parity."""

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
        "DUAL-03-01",
        "真实双通道流量波形（GPU Bezier）",
        "TrafficSample/TrafficWaveformSnapshot",
        "parity-ready",
    )
    require(
        violations,
        "docs/CORE_030_REARCHITECTURE.md",
        "DUAL-03-01",
        "TrafficWaveformApplication",
        "cubic-Bezier",
    )
    require(
        violations,
        "docs/DUAL_SURFACE_ARCHITECTURE_AUDIT_030.md",
        "DUAL-03-01",
        "generation reset",
        "host-verified",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/traffic_waveform.rs",
        "TRAFFIC_WAVEFORM_CAPACITY",
        "TrafficSample",
        "TrafficWaveformSnapshot",
        "is_drawable",
    )
    require(
        violations,
        "crates/infiltrator-domain/src/traffic_waveform.rs",
        "TrafficWaveformBuffer",
        "record",
        "smooth_series",
        "smooth_dual_series",
        "display_series",
        "buffer_is_bounded_and_resets_on_generation_change",
        "stopped_core_does_not_append_a_fake_zero_sample",
    )
    require(
        violations,
        "crates/infiltrator-application/src/traffic_waveform_application.rs",
        "TrafficWaveformApplication",
        "application_records_only_live_core_samples",
    )
    require(
        violations,
        "crates/infiltrator-application/src/surface_reader.rs",
        "traffic_waveform",
        "TrafficWaveformApplication::new",
        "self.traffic_waveform.record",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/surface_snapshot.rs",
        "pub traffic_waveform: crate::traffic_waveform::TrafficWaveformSnapshot",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/history.rs",
        "chart_inputs",
        "projection.traffic_waveform",
        "infiltrator_domain::traffic_waveform::display_series",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/overview.rs",
        "chart_inputs",
        "chart_scene_with_smooth",
        "with_smooth(smooth)",
    )
    require(
        violations,
        "crates/infiltrator-bevy-widgets/src/chart.rs",
        "chart_scene_with_smooth",
        "dense shared",
        "Bezier values",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/overview_tests.rs",
        "live_surface_waveform_uses_shared_bezier_value_projection",
        "TrafficWaveformSnapshot",
        "TrafficSample",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/state.rs",
        "traffic_waveform",
        "snapshot.traffic_waveform",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/overview.rs",
        "state.runtime.traffic_waveform",
        "TrafficChart",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/waveform.rs",
        "TrafficWaveformSnapshot",
        "smooth_dual_series",
        "raw_series",
        "shared_live_samples_are_the_input_to_the_same_bezier_projection",
    )
    require(
        violations,
        "scripts/test.sh",
        "traffic-waveform-guard.py --mode enforce",
    )
    require(
        violations,
        "scripts/test-bevy.sh",
        "traffic-waveform-guard.py\" --mode enforce",
    )

    if violations:
        for violation in violations:
            print(f"traffic-waveform-guard: {violation}", file=sys.stderr)
        print(f"traffic-waveform-guard: violations={len(violations)}", file=sys.stderr)
        return 1 if args.mode == "enforce" else 0
    print("traffic-waveform-guard: DUAL-03-01 markers=complete violations=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
