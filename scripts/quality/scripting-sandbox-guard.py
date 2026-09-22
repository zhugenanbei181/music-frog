#!/usr/bin/env python3
"""Fail-closed guard for the group 10 scripting-sandbox ledger (DUAL-10-01..15).

Group 10's closure standard matches the rest of the master plan: one shared
contract/application source, both surfaces, dual headless tests, and an honest
per-item ledger. This guard asserts:

* the per-item ledger rows exist with the status vocabulary the master plan
  uses, and the batch notes name the closed items;
* the shared domain keeps the real Mixin reductions (preflight, preset
  toggles, cascade pipeline preview, three-column editor model) and the shared
  application keeps the scripting-sandbox regression matrix;
* the preflight gate is the one both surfaces call before a Mixin overlay may
  be committed (a YAML-only check is not enough);
* the Iced Mixin pane and the Bevy editor card render the shared studio facts,
  the three-column Base/overlay/composed workspace, and route toggles back
  through the shared codec;
* the shared script-sandbox read model is real (matched directives, console
  logs, input/output YAML, breaker state) and both consoles render the same
  projection published through `SurfaceSnapshot.script_sandbox`;
* the export pipeline is real and honest: composed `.yaml`/`.js`/JSON
  artifacts, a host save-file port with a typed unsupported answer, the
  published `SurfaceSnapshot.script_export`, and both surfaces rendering it;
* the honest gaps do not regress into a fabricated QuickJS claim: there is no
  bundled QuickJS engine, the read model reports a directive DSL, the ledger
  states that a desktop file dialog does not exist, and a real JS engine stays
  a recorded product decision.
"""

from __future__ import annotations

import argparse
import pathlib
import sys


ROOT = pathlib.Path(__file__).resolve().parents[2]

LEDGER = "docs/DUAL_SURFACE_PARITY_MASTER_PLAN.md"
DOMAIN_STUDIO = "crates/infiltrator-domain/src/mixin_studio.rs"
DOMAIN_MIXIN = "crates/infiltrator-domain/src/mixin.rs"
DOMAIN_DIRECTIVES = "crates/infiltrator-domain/src/script_engine_directives.rs"
DOMAIN_EXPORT = "crates/infiltrator-domain/src/script_export.rs"
CONTRACT_SANDBOX = "crates/infiltrator-contract/src/script_sandbox.rs"
CONTRACT_MATRIX = "crates/infiltrator-contract/src/script_sandbox_matrix.rs"
CONTRACT_EXPORT = "crates/infiltrator-contract/src/script_export.rs"
CONTRACT_SURFACE = "crates/infiltrator-contract/src/surface_snapshot.rs"
PORTS_EXPORT = "crates/infiltrator-ports/src/script_export.rs"
PORTS_HOST_RUNTIME = "crates/infiltrator-ports/src/host_runtime.rs"
APP_SCRIPT = "crates/infiltrator-application/src/script_application.rs"
APP_EXPORT = "crates/infiltrator-application/src/script_export_application.rs"
APP_EXPORT_TEST = "crates/infiltrator-application/src/script_export_application_test.rs"
APP_READER = "crates/infiltrator-application/src/surface_reader.rs"
APP_MATRIX = "crates/infiltrator-application/src/script_sandbox_matrix_application.rs"
APP_MATRIX_TEST = "crates/infiltrator-application/src/script_sandbox_matrix_application_test.rs"
DESKTOP_EXPORT = "crates/infiltrator-desktop/src/script_export.rs"
DESKTOP_RUNTIME = "crates/infiltrator-desktop/src/runtime.rs"
ICED_VIEW = "crates/infiltrator-iced/src/view/mixin_studio.rs"
ICED_CONSOLE = "crates/infiltrator-iced/src/view/script_console.rs"
ICED_EXPORT_VIEW = "crates/infiltrator-iced/src/view/script_export.rs"
ICED_EXPORT_UPDATE = "crates/infiltrator-iced/src/update/script_export.rs"
ICED_OPTIONS = "crates/infiltrator-iced/src/update/profile/options.rs"
ICED_MESSAGE = "crates/infiltrator-iced/src/types/message.rs"
ICED_MATRIX_TEST = "crates/infiltrator-iced/tests/headless/scripting_matrix_tests.rs"
ICED_EXPORT_TEST = "crates/infiltrator-iced/tests/gui/business_flow/options_editors.rs"
BEVY_STUDIO = "crates/infiltrator-bevy-ui/src/pages/profiles_editor_mixin_studio.rs"
BEVY_SYNC = "crates/infiltrator-bevy-ui/src/pages/profiles_editor_panes_sync.rs"
BEVY_SCRIPT = "crates/infiltrator-bevy-ui/src/pages/profiles_script.rs"
BEVY_PROJECTION = "crates/infiltrator-bevy-ui/src/surface_projection.rs"
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
        "2026-09-23 组 10 批次 B",
        "2026-09-23 组 10 批次 C",
        "parity-ready",
        "shared-ready",
        "planned",
        # Honest facts the ledger must keep stating.
        "无真实 QuickJS 引擎",
        "指令 DSL",
        "桌面宿主没有原生保存对话框",
        "SurfaceSnapshot.script_sandbox",
        "SurfaceSnapshot.script_export",
        # Closed items carry their distinctive evidence tokens.
        "mixin_studio",
        "preflight_mixin",
        "MIXIN_PRESET_TOGGLES",
        "preview_cascade",
        "mixin_editor_columns",
        "three_column_row",
        "three_column_scene",
        "ScriptExportPort",
        "script_export_port",
        "DesktopScriptExportPort",
        "compose_directive_dsl_export",
        "check_three_column_editor",
        "check_extension_export",
        "ScriptSandboxMatrixReport",
        "run_deterministic_matrix",
        "matched_directives",
        "check_console_read_model",
        "check_dual_surface_alignment",
    )

    # 2. The shared domain keeps the real Mixin reductions and reports the
    #    directives that really matched.
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
        "pub struct MixinColumn",
        "pub struct MixinEditorColumns",
        "pub fn mixin_editor_columns",
        "pub fn is_composed",
        "pub fn is_blocked",
    )
    require(
        violations,
        DOMAIN_MIXIN,
        "pub struct CascadeOverlayPipeline",
        "pub fn merge_profile_with_config_fidelity",
    )
    require(
        violations,
        DOMAIN_DIRECTIVES,
        "ScriptDirectiveAudit",
        "evaluate_ast_directives",
        "audits.push",
    )
    require(
        violations,
        DOMAIN_EXPORT,
        "pub struct ExportArtifact",
        "pub fn sanitize_file_stem",
        "pub fn compose_mixin_overlay_export",
        "pub fn compose_directive_dsl_export",
        "pub fn compose_extension_package_export",
        "not JavaScript",
    )
    require(violations, DOMAIN_LIB, "pub mod mixin_studio;", "pub mod script_export;")

    # 3. The shared contract read model: honest engine kind, matched
    #    directives, breaker state, and the surface publication.
    require(
        violations,
        CONTRACT_SANDBOX,
        "pub enum ScriptEngineKind",
        "pub struct ScriptDirectiveMatch",
        "pub struct ScriptCircuitBreakerSnapshot",
        "fn is_real_javascript",
        "pub matched_directives: Vec<ScriptDirectiveMatch>",
        "pub circuit_breaker: ScriptCircuitBreakerSnapshot",
        "pub hook_stage: String",
        "pub engine_kind: ScriptEngineKind",
    )
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
    require(
        violations,
        CONTRACT_EXPORT,
        "pub const DIRECTIVE_DSL_JS_HEADER",
        "pub enum ScriptExportKind",
        "pub enum ScriptExportOutcome",
        "pub struct ScriptExportRequest",
        "pub struct ScriptExportReceipt",
        "pub struct ScriptExportSnapshot",
        "pub const fn is_javascript",
        "pub fn content_preview",
        "不是 JavaScript",
    )
    require(
        violations,
        CONTRACT_SURFACE,
        "pub script_export: Option<crate::script_export::ScriptExportSnapshot>",
    )
    require(violations, CONTRACT_LIB, "pub mod script_export;", "pub mod script_sandbox_matrix;")

    # 4. The shared application projection publishes the one read model and the
    #    one export pipeline with a host save-file port.
    require(
        violations,
        APP_SCRIPT,
        "pub fn last_script_sandbox",
        "pub fn publish_script_sandbox",
        "pub fn run_sandbox_at_stage",
        "ScriptEngineKind::DirectiveDsl",
        "fn project_directives",
        "fn stage_for_preset",
        "fn breaker_snapshot",
    )
    require(
        violations,
        APP_EXPORT,
        "pub struct ScriptExportApplication",
        "pub fn without_host_port",
        "pub fn export_mixin_overlay",
        "pub fn export_directive_dsl",
        "pub fn export_preset",
        "pub fn export_extension_package",
        "pub fn export_draft_package",
        "pub fn last_script_export",
        "pub fn publish_script_export",
        "PortError::Unsupported",
    )
    require(
        violations,
        APP_EXPORT_TEST,
        "a_host_with_a_save_port_persists_the_real_artifact_and_reports_the_path",
        "a_host_without_a_save_port_reports_typed_unsupported_without_losing_content",
        "invalid_drafts_are_refused_before_any_port_call",
    )
    require(violations, PORTS_EXPORT, "pub trait ScriptExportPort", "UnsupportedScriptExportPort", "PortError::unsupported")
    require(violations, PORTS_HOST_RUNTIME, "fn script_export_port", "ScriptExportPort")
    require(violations, APP_READER, "crate::script_application::last_script_sandbox()", "crate::script_export_application::last_script_export()")
    require(
        violations,
        APP_MATRIX,
        "pub struct ScriptSandboxMatrixApplication",
        "pub fn run_deterministic_matrix",
        "DUAL-10-08",
        "DUAL-10-10",
        "DUAL-10-11",
        "DUAL-10-14",
        "fn check_circuit_breaker",
        "fn check_cascade_pipeline",
        "fn check_mixin_preflight",
        "fn check_console_read_model",
        "fn check_dual_surface_alignment",
        "fn check_three_column_editor",
        "fn check_extension_export",
        "fn check_extension_round_trip",
        "ScriptExportApplication",
        "无真实 QuickJS 引擎",
    )
    # The application layer stays executor-neutral.
    forbid(violations, APP_MATRIX, "tokio::", "reqwest::")
    forbid(violations, APP_EXPORT, "tokio::", "reqwest::")
    require(
        violations,
        APP_LIB,
        "pub mod script_export_application;",
        "pub mod script_sandbox_matrix_application;",
    )
    require(
        violations,
        APP_MATRIX_TEST,
        "deterministic_matrix_passes_every_covered_item_and_names_the_planned_ones",
        "matrix_is_deterministic_across_runs",
        "assert_eq!(report.covered_count(), 14)",
    )

    # 5. The desktop host writes a real file and never fabricates a path.
    require(
        violations,
        DESKTOP_EXPORT,
        "pub struct DesktopScriptExportPort",
        "pub const MAX_EXPORT_BYTES",
        "impl ScriptExportPort for DesktopScriptExportPort",
        "fs::write",
        "a_real_file_lands_in_the_exports_directory",
        "a_path_traversal_name_is_refused_before_any_write",
    )
    require(violations, DESKTOP_RUNTIME, "fn script_export_port", "DesktopScriptExportPort")

    # 6. Iced: the Mixin pane renders the shared studio (three columns) and
    #    gates on the shared preflight; the console renders the shared sandbox
    #    projection, loads presets from the shared catalogue, and routes the
    #    real export through the shared application + host port.
    require(
        violations,
        ICED_VIEW,
        "pub fn toggle_row",
        "pub fn preflight_banner",
        "pub fn cascade_strip",
        "pub fn three_column_row",
        "mixin_editor_columns",
        "MIXIN_PRESET_TOGGLES",
        "preflight_mixin",
        "preview_cascade",
        "Message::ToggleMixinPreset",
    )
    require(
        violations,
        ICED_CONSOLE,
        "fn console_body",
        "matched_directives",
        "script_sandbox_engine",
        "builtin_presets",
        "script_sandbox.snapshot.as_ref()",
        "script_export::export_section",
    )
    require(
        violations,
        ICED_EXPORT_VIEW,
        "pub fn export_section",
        "ScriptExportKind",
        "script_export_title",
        "snapshot.content_preview",
        "Message::ExportScriptDraft",
    )
    require(
        violations,
        ICED_EXPORT_UPDATE,
        "pub(crate) fn export_script_draft",
        "ScriptExportApplication",
        "script_export_port",
        "Message::ScriptExportFinished",
    )
    require(
        violations,
        ICED_OPTIONS,
        "Message::ToggleMixinPreset(id, enabled)",
        "mixin_studio::set_toggle",
        "fn mixin_preflight",
        "mixin_studio::preflight_mixin",
    )
    require(
        violations,
        ICED_MESSAGE,
        "ToggleMixinPreset(String, bool)",
        "ExportScriptDraft(infiltrator_contract::script_export::ScriptExportKind)",
        "ScriptExportFinished(",
    )
    require(
        violations,
        ICED_MATRIX_TEST,
        "script_sandbox_matrix_passes_on_the_iced_surface",
        "shared_console_projection_carries_every_wired_fact_for_both_surfaces",
        "three_column_editor_and_export_ride_the_shared_reduction",
        "assert_eq!(report.covered_passed_count(), 14)",
    )
    require(
        violations,
        ICED_EXPORT_TEST,
        "mixin_export_writes_a_real_yaml_file_and_reports_the_host_outcome",
        "DesktopScriptExportPort",
        "Message::ExportScriptDraft",
        "alpha.mixin.yaml",
    )

    # 7. Bevy: same shared facts, the three-column workspace, toggles routed
    #    through the shared codec and the shared preflight gating the commit;
    #    the console renders the same sandbox and export projections.
    require(
        violations,
        BEVY_STUDIO,
        "pub struct MixinToggleButton",
        "pub struct MixinStudioBody",
        "pub fn mixin_studio_scene",
        "pub fn cascade_line",
        "pub fn on_mixin_toggle_activated",
        "pub fn refresh_mixin_studio_body",
        "fn three_column_scene",
        "pub fn column_caption_zh",
        "pub struct MixinComposedText",
        "pub struct MixinComposedErrorText",
        "mixin_studio::MIXIN_PRESET_TOGGLES",
        "mixin_studio::set_toggle",
        "preview_cascade_from_yaml",
        "mixin_editor_columns",
        "last_rendered = u64::MAX",
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
        BEVY_SCRIPT,
        "pub struct ScriptSandboxBody",
        "pub struct ScriptSandboxViewState",
        "pub fn script_sandbox_body",
        "pub fn script_sandbox_scene",
        "pub fn rebuild_script_sandbox_body",
        "fn export_rows",
        "已执行指令",
        "安全降级",
        "熔断状态",
        "builtin_presets",
        "script_export",
        "宿主结果",
    )
    require(
        violations,
        BEVY_PROJECTION,
        "script_sandbox: snapshot.script_sandbox.clone()",
        "script_export: snapshot.script_export.clone()",
    )
    require(
        violations,
        BEVY_MATRIX_TEST,
        "test_profiles_mixin_studio_renders_shared_preflight_toggles_and_cascade",
        "test_script_sandbox_matrix_passes_on_the_bevy_surface",
        "test_profiles_script_console_renders_the_shared_projection",
        "test_profiles_script_console_renders_the_shared_export_projection",
        "安全降级：原配置保持不变",
        "Base 配置",
        "合成后最终配置",
    )

    # 8. The honest gap: no bundled JavaScript engine may be claimed.
    forbid(
        violations,
        DOMAIN_STUDIO,
        "quickjs::",
        "rquickjs",
        "boa_engine",
    )
    forbid(
        violations,
        DOMAIN_DIRECTIVES,
        "quickjs::",
        "rquickjs",
        "boa_engine",
    )
    forbid(violations, CONTRACT_SANDBOX, "quickjs::", "rquickjs", "boa_engine")
    forbid(violations, DOMAIN_EXPORT, "quickjs::", "rquickjs", "boa_engine")
    forbid(violations, CONTRACT_EXPORT, "quickjs::", "rquickjs", "boa_engine")
    forbid(violations, APP_EXPORT, "quickjs::", "rquickjs", "boa_engine")
    forbid(violations, PORTS_EXPORT, "quickjs::", "rquickjs", "boa_engine")

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
