#!/usr/bin/env python3
"""Fail-closed guard for DUAL-03-02 dynamic traffic scale parity."""

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
        "DUAL-03-02",
        "动态量程标尺与发光着色器",
        "TrafficScaleSnapshot",
        "parity-ready",
    )
    require(
        violations,
        "docs/CORE_030_REARCHITECTURE.md",
        "DUAL-03-02",
        "TrafficScaleApplication",
        "5% headroom",
    )
    require(
        violations,
        "docs/DUAL_SURFACE_ARCHITECTURE_AUDIT_030.md",
        "DUAL-03-02",
        "TrafficScaleSnapshot/TrafficScaleApplication",
        "host-verified",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/traffic_scale.rs",
        "TrafficRateUnit",
        "KibiBytes",
        "MebiBytes",
        "GibiBytes",
        "TrafficScaleSnapshot",
        "max_bps",
        "ticks",
        "format_max",
    )
    require(
        violations,
        "crates/infiltrator-domain/src/traffic_scale.rs",
        "SCALE_HEADROOM",
        "compute",
        "compute_from_peak",
        "compute_from_rates",
        "scale_uses_the_larger_channel_and_selects_a_human_unit",
        "zero_nonfinite_and_negative_rates_have_a_safe_baseline",
    )
    require(
        violations,
        "crates/infiltrator-application/src/traffic_scale_application.rs",
        "TrafficScaleApplication",
        "pub fn compute",
        "application_exposes_one_scale_for_both_channels",
    )
    require(
        violations,
        "crates/infiltrator-application/src/surface_reader.rs",
        "traffic_scale",
        "TrafficScaleApplication",
        "self.traffic_scale.compute",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/surface_snapshot.rs",
        "pub traffic_scale: crate::traffic_scale::TrafficScaleSnapshot",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/history.rs",
        "TrafficScaleSnapshot",
        "projection.traffic_scale",
        "chart_inputs",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/overview.rs",
        "scale_line_scene",
        "format_scale",
        "chart_scene_with_scale",
        "Some(scale.max_bps as f32)",
    )
    require(
        violations,
        "crates/infiltrator-bevy-widgets/src/chart.rs",
        "chart_scene_with_scale",
        "ScaleMode::Fixed",
        "layer.line, 0.14",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/overview_tests.rs",
        "live_surface_waveform_uses_shared_bezier_value_projection",
        "traffic_scale",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/state.rs",
        "pub traffic_scale",
        "snapshot.traffic_scale",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/overview.rs",
        "traffic_scale",
        "overview_scale_max",
        "scale: Some(scale)",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/waveform.rs",
        "TrafficScaleSnapshot",
        "pub scale",
        "resolved_scale",
        "compute_from_rates",
        "with_color(Color { a: 0.16",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/waveform_tests.rs",
        "canvas_uses_the_application_scale_instead_of_a_fixed_floor",
    )
    require(
        violations,
        "scripts/test.sh",
        "traffic-scale-guard.py --mode enforce",
    )
    require(
        violations,
        "scripts/test-bevy.sh",
        "traffic-scale-guard.py\" --mode enforce",
    )

    if violations:
        for violation in violations:
            print(f"traffic-scale-guard: {violation}", file=sys.stderr)
        print(f"traffic-scale-guard: violations={len(violations)}", file=sys.stderr)
        return 1 if args.mode == "enforce" else 0
    print("traffic-scale-guard: DUAL-03-02 markers=complete violations=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
