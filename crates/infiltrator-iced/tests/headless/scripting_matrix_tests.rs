//! DUAL-10-15: the shared scripting-sandbox regression matrix asserted on the
//! Iced surface, plus DUAL-10-05/14: the shared console projection the Iced
//! pane renders and publishes for the Bevy surface.
//! test-intent: behavior

use infiltrator_application::script_application::{
    ScriptApplication, last_script_sandbox, publish_script_sandbox,
};
use infiltrator_application::script_export_application::{
    ScriptExportApplication, last_script_export, publish_script_export,
};
use infiltrator_application::script_sandbox_matrix_application::ScriptSandboxMatrixApplication;
use infiltrator_contract::script_export::{ScriptExportKind, ScriptExportOutcome};
use infiltrator_domain::mixin_studio;

#[test]
fn script_sandbox_matrix_passes_on_the_iced_surface() {
    let report = ScriptSandboxMatrixApplication::run_deterministic_matrix();
    assert_eq!(report.scenarios.len(), 15);
    assert!(
        report.all_covered_passed(),
        "failed covered rows: {:?}",
        report.failed_ids()
    );
    // Only the honest QuickJS gap stays uncovered.
    assert_eq!(report.not_covered_ids(), vec!["DUAL-10-01"]);
    assert_eq!(report.covered_passed_count(), 14);
    assert!(report.summary_zh().contains("14/14"));
}

/// DUAL-10-09/12 on the Iced surface: the three-column model is the shared
/// reduction (real base, editable overlay, real composed output) and the
/// per-surface export is the real artifact with a typed host outcome.
#[test]
fn three_column_editor_and_export_ride_the_shared_reduction() {
    let base = "mode: rule\nport: 7890\n";
    let columns = mixin_studio::mixin_editor_columns(base, "mode: global\n");
    assert_eq!(columns.base.content, base);
    assert!(columns.overlay.editable);
    assert!(columns.is_composed());
    assert!(columns.composed.content.contains("mode: global"));
    assert!(columns.composed.content.contains("port: 7890"));
    let blocked = mixin_studio::mixin_editor_columns(base, "mode: [bad\n");
    assert!(blocked.is_blocked());
    assert!(blocked.composed.content.is_empty());

    // A host with no save-file port is a typed unsupported, not a fake path.
    let hostless = ScriptExportApplication::without_host_port()
        .export_directive_dsl(
            Some("iced"),
            "function main(config) {\n  auto_country_groups(config);\n  return config;\n}",
            Some("auto-country-groups"),
        )
        .expect("compose");
    assert_eq!(hostless.kind, ScriptExportKind::DirectiveDslScript);
    assert_eq!(hostless.file_name, "iced.js");
    assert!(hostless.content.contains("不是 JavaScript"));
    assert!(hostless.outcome.is_unsupported());
    assert!(matches!(
        hostless.outcome,
        ScriptExportOutcome::Unsupported { .. }
    ));
    // The projection the Bevy console reads is the same fact.
    publish_script_export(hostless.clone());
    assert_eq!(last_script_export().as_ref(), Some(&hostless));
}

#[test]
fn shared_console_projection_carries_every_wired_fact_for_both_surfaces() {
    let application = ScriptApplication::new();
    let script = "function main(config, profile) {\n  console.log(\"hello console\");\n  auto_country_groups(config);\n  return config;\n}";
    let yaml = "proxies:\n  - name: 🇭🇰 HK 01\n    type: ss\n  - name: 🇯🇵 JP 01\n    type: ss\n";
    let snapshot = application.run_sandbox(script, yaml, Some("auto-country-groups"));

    // DUAL-10-02: real hook stage from the shared preset.
    assert_eq!(snapshot.hook_stage, "pre_merge");
    assert!(snapshot.hook_stage_label.contains("Pre-Merge"));
    // DUAL-10-03: the breaker limits travel with the projection.
    assert_eq!(snapshot.timeout_limit_ms, 500);
    assert_eq!(snapshot.max_memory_limit_bytes, 64 * 1024 * 1024);
    assert_eq!(snapshot.circuit_breaker.failure_threshold, 3);
    assert!(!snapshot.is_circuit_tripped());
    // DUAL-10-06: streamed console capture.
    assert!(
        snapshot
            .console_logs
            .iter()
            .any(|entry| entry.message.contains("hello console"))
    );
    // DUAL-10-07: before/after preview.
    assert!(snapshot.transformed_yaml.is_some());
    assert!(
        snapshot
            .diff
            .as_ref()
            .is_some_and(|diff| diff.has_differences())
    );
    // DUAL-10-05/14: only the matched directive is listed and the projection
    // is the one the Bevy surface reads.
    assert_eq!(snapshot.matched_directive_count(), 1);
    assert_eq!(snapshot.matched_directives[0].id, "auto_country_groups");
    assert!(!snapshot.engine_kind.is_real_javascript());
    // DUAL-10-01: the capability negotiation travels with the projection and
    // states the bundled engine's real limit (no JS syntax).
    assert!(!snapshot.engine_capabilities.supports_javascript_syntax);
    assert!(snapshot.engine_capabilities.supports_directive_dsl);
    assert!(snapshot.engine_kind_matches_capabilities());
    publish_script_sandbox(snapshot.clone());
    assert_eq!(last_script_sandbox().as_ref(), Some(&snapshot));
}

/// DUAL-10-01: the Iced console renders whatever engine the shared read model
/// reports. A directive-DSL snapshot shows the honest "no JS syntax" limit; a
/// snapshot whose engine negotiated the JavaScript slot renders the JS label
/// through the same helper, so the swap needs no view change.
#[test]
fn iced_console_renders_the_reported_engine_and_its_capability_limits() {
    use infiltrator_contract::script_sandbox::{
        ScriptEngineCapabilities, ScriptEngineKind, ScriptSandboxSnapshot,
    };
    use infiltrator_iced::view::script_console::engine_meta_rows;
    use infiltrator_shared::locales::Lang;

    let dsl = ScriptSandboxSnapshot::demo_fixture();
    let (engine_zh, capabilities_zh) = engine_meta_rows(&Lang("zh-CN"), &dsl);
    assert!(engine_zh.contains("指令 DSL"));
    assert!(capabilities_zh.contains("不支持 JavaScript 语法"));
    let (engine_en, capabilities_en) = engine_meta_rows(&Lang("en-US"), &dsl);
    assert!(engine_en.contains("Directive DSL"));
    assert!(capabilities_en.contains("No JavaScript syntax"));

    // Same helper, a read model produced by a JavaScript-capable engine: the
    // surface needs no change to report it honestly.
    let mut js = ScriptSandboxSnapshot::demo_fixture();
    js.engine_kind = ScriptEngineKind::JavascriptEngine;
    js.engine_capabilities = ScriptEngineCapabilities {
        supports_javascript_syntax: true,
        supports_directive_dsl: false,
        ..ScriptEngineCapabilities::directive_dsl()
    };
    assert!(js.engine_kind_matches_capabilities());
    let (engine_zh, capabilities_zh) = engine_meta_rows(&Lang("zh-CN"), &js);
    assert!(engine_zh.contains("JavaScript 引擎"));
    assert!(capabilities_zh.contains("支持 JavaScript"));
    let (engine_en, capabilities_en) = engine_meta_rows(&Lang("en-US"), &js);
    assert!(engine_en.contains("ECMAScript"));
    assert!(capabilities_en.contains("JavaScript syntax supported"));
}

#[test]
fn safe_degradation_projection_names_no_directive_on_failure() {
    let application = ScriptApplication::new();
    let input = "port: 7890\n";
    let snapshot = application.run_sandbox(
        "function main(config) { remove_rules(config, \"([\"); return config; }",
        input,
        None,
    );
    assert!(snapshot.has_error());
    assert!(snapshot.error_detail.is_some());
    assert_eq!(snapshot.input_yaml, input);
    assert!(snapshot.transformed_yaml.is_none());
    assert_eq!(snapshot.matched_directive_count(), 0);
}
