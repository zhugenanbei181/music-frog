#!/usr/bin/env python3
"""Fail-closed guard for the group 10 scripting-sandbox ledger (DUAL-10-01..15).

Group 10's closure standard matches the rest of the master plan: one shared
contract/application source, both surfaces, dual headless tests, and an honest
per-item ledger. This guard asserts:

* the per-item ledger rows exist with the status vocabulary the master plan
  uses, and the batch note names the closed items;
* the shared domain keeps the real Mixin reductions (preflight, preset
  toggles, cascade pipeline preview) and the shared application keeps the
  scripting-sandbox regression matrix;
* the preflight gate is the one both surfaces call before a Mixin overlay may
  be committed (a YAML-only check is not enough);
* the Iced Mixin pane and the Bevy editor card render the shared studio facts
  and route toggles back through the shared codec;
* the honest gaps do not regress into a fabricated QuickJS claim: there is no
  bundled QuickJS engine and the ledger says so.
"""

from __future__ import annotations

import argparse
import pathlib
import sys


ROOT = pathlib.Path(__file__).resolve().parents[2]

LEDGER = "docs/DUAL_SURFACE_PARITY_MASTER_PLAN.md"
DOMAIN_STUDIO = "crates/infiltrator-domain/src/mixin_studio.rs"
DOMAIN_MIXIN = "crates/infiltrator-domain/src/mixin.rs"
CONTRACT_MATRIX = "crates/infiltrator-contract/src/script_sandbox_matrix.rs"
APP_MATRIX = "crates/infiltrator-application/src/script_sandbox_matrix_application.rs"
APP_MATRIX_TEST = "crates/infiltrator-application/src/script_sandbox_matrix_application_test.rs"
ICED_VIEW = "crates/infiltrator-iced/src/view/mixin_studio.rs"
ICED_OPTIONS = "crates/infiltrator-iced/src/update/profile/options.rs"
ICED_MESSAGE = "crates/infiltrator-iced/src/types/message.rs"
ICED_MATRIX_TEST = "crates/infiltrator-iced/tests/headless/scripting_matrix_tests.rs"
BEVY_STUDIO = "crates/infiltrator-bevy-ui/src/pages/profiles_editor_mixin_studio.rs"
BEVY_SYNC = "crates/infiltrator-bevy-ui/src/pages/profiles_editor_panes_sync.rs"
BEVY_MATRIX_TEST = "crates/infiltrator-bevy-ui/tests/headless/pages_matrix_a_tests.rs"
DOMAIN_LIB = "crates/infiltrator-domain/src/lib.rs"
CONTRACT_LIB = "crates/infiltrator-contract/src/lib.rs"
APP_LIB = "crates/infiltrator-application/src/lib.rs"


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
            violations.append(f"{path} still contains forbidden marker {marker!r}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--mode", choices=["report", "enforce"], default="enforce")
    args = parser.parse_args()
    violations: list[str] = []

    # 1. The full per-item ledger, with the plan's status vocabulary.
    require(
        violations,
        LEDGER,
        "DUAL-10-01",
        "DUAL-10-02",
        "DUAL-10-03",
        "DUAL-10-04",
        "DUAL-10-05",
        "DUAL-10-06",
        "DUAL-10-07",
        "DUAL-10-08",
        "DUAL-10-09",
        "DUAL-10-10",
        "DUAL-10-11",
        "DUAL-10-12",
        "DUAL-10-13",
        "DUAL-10-14",
        "DUAL-10-15",
        "组 10 逐项账目",
        "2026-09-22 组 10 批次 A",
        "parity-ready",
        "shared-ready",
        "planned",
        # Honest facts the ledger must keep stating.
        "无真实 QuickJS 引擎",
        "脚本控制台仍未镜像",
        "分页式而非 Base/Mixin/合成三栏",
        # Closed items carry their distinctive evidence tokens.
        "mixin_studio",
        "preflight_mixin",
        "MIXIN_PRESET_TOGGLES",
        "preview_cascade",
        "ScriptSandboxMatrixReport",
        "run_deterministic_matrix",
    )

    # 2. The shared domain keeps the real Mixin reductions.
    require(
        violations,
        DOMAIN_STUDIO,
        "pub struct MixinPreflightReport",
        "pub fn preflight_mixin",
        "pub fn is_blocking",
        "pub struct MixinPresetToggle",
        "pub const MIXIN_PRESET_TOGGLES",
        "pub fn set_toggle",
        "pub fn toggle_enabled",
        "pub struct CascadeStageReport",
        "pub struct CascadeOverlayReport",
        "pub fn preview_cascade",
        "pub fn preview_cascade_from_yaml",
    )
    require(
        violations,
        DOMAIN_MIXIN,
        "pub struct CascadeOverlayPipeline",
        "pub fn merge_profile_with_config_fidelity",
    )
    require(violations, DOMAIN_LIB, "pub mod mixin_studio;")

    # 3. Shared contract + application: the scripting-sandbox matrix.
    require(
        violations,
        CONTRACT_MATRIX,
        "pub struct ScriptSandboxMatrixScenario",
        "pub struct ScriptSandboxMatrixReport",
        "pub fn all_covered_passed",
        "pub fn not_covered_ids",
        "pub fn covered_passed_count",
        "pub fn summary_zh",
    )
    require(violations, CONTRACT_LIB, "pub mod script_sandbox_matrix;")
    require(
        violations,
        APP_MATRIX,
        "pub struct ScriptSandboxMatrixApplication",
        "pub fn run_deterministic_matrix",
        "DUAL-10-08",
        "DUAL-10-10",
        "DUAL-10-11",
        "DUAL-10-15",
        "fn check_circuit_breaker",
        "fn check_cascade_pipeline",
        "fn check_mixin_preflight",
        "无真实 QuickJS 引擎",
    )
    # The application layer stays executor-neutral.
    forbid(violations, APP_MATRIX, "tokio::", "reqwest::")
    require(violations, APP_LIB, "pub mod script_sandbox_matrix_application;")
    require(
        violations,
        APP_MATRIX_TEST,
        "deterministic_matrix_passes_every_covered_item_and_names_the_planned_ones",
        "matrix_is_deterministic_across_runs",
        "assert_eq!(report.covered_count(), 11)",
    )

    # 4. Iced: the pane renders the shared studio and gates on the shared
    #    preflight (the same rule the application re-runs on commit).
    require(
        violations,
        ICED_VIEW,
        "pub fn toggle_row",
        "pub fn preflight_banner",
        "pub fn cascade_strip",
        "MIXIN_PRESET_TOGGLES",
        "preflight_mixin",
        "preview_cascade",
        "Message::ToggleMixinPreset",
    )
    require(
        violations,
        ICED_OPTIONS,
        "Message::ToggleMixinPreset(id, enabled)",
        "mixin_studio::set_toggle",
        "fn mixin_preflight",
        "mixin_studio::preflight_mixin",
    )
    require(violations, ICED_MESSAGE, "ToggleMixinPreset(String, bool)")
    require(
        violations,
        ICED_MATRIX_TEST,
        "script_sandbox_matrix_passes_on_the_iced_surface",
    )

    # 5. Bevy: same shared facts, toggles routed through the shared codec and
    #    the shared preflight gating the commit.
    require(
        violations,
        BEVY_STUDIO,
        "pub struct MixinToggleButton",
        "pub struct MixinStudioBody",
        "pub fn mixin_studio_scene",
        "pub fn cascade_line",
        "pub fn on_mixin_toggle_activated",
        "pub fn refresh_mixin_studio_body",
        "mixin_studio::MIXIN_PRESET_TOGGLES",
        "mixin_studio::set_toggle",
        "preview_cascade_from_yaml",
    )
    require(
        violations,
        BEVY_SYNC,
        "mixin_studio::preflight_mixin",
        "共享预检",
        "on_mixin_toggle_activated",
        "refresh_mixin_studio_body",
    )
    require(
        violations,
        BEVY_MATRIX_TEST,
        "test_profiles_mixin_studio_renders_shared_preflight_toggles_and_cascade",
        "test_script_sandbox_matrix_passes_on_the_bevy_surface",
    )

    # 6. The honest gap: no bundled JavaScript engine may be claimed.
    forbid(
        violations,
        DOMAIN_STUDIO,
        "quickjs::",
        "rquickjs",
        "boa_engine",
    )

    if violations:
        for violation in violations:
            print(f"scripting-sandbox-guard: {violation}")
        if args.mode == "enforce":
            print("scripting-sandbox-guard: FAIL")
            return 1
        print("scripting-sandbox-guard: report mode, violations above")
        return 0
    print("scripting-sandbox-guard: ok (group 10 DUAL-10-01..15)")
    return 0


if __name__ == "__main__":
    sys.exit(main())