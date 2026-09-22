//! DUAL-10-15: the shared scripting-sandbox regression matrix asserted on the
//! Iced surface, plus DUAL-10-05/14: the shared console projection the Iced
//! pane renders and publishes for the Bevy surface.
//! test-intent: behavior

use infiltrator_application::script_application::{
    ScriptApplication, last_script_sandbox, publish_script_sandbox,
};
use infiltrator_application::script_sandbox_matrix_application::ScriptSandboxMatrixApplication;

#[test]
fn script_sandbox_matrix_passes_on_the_iced_surface() {
    let report = ScriptSandboxMatrixApplication::run_deterministic_matrix();
    assert_eq!(report.scenarios.len(), 15);
    assert!(
        report.all_covered_passed(),
        "failed covered rows: {:?}",
        report.failed_ids()
    );
    assert_eq!(
        report.not_covered_ids(),
        vec!["DUAL-10-01", "DUAL-10-09"],
        "the honest gaps stay uncovered"
    );
    assert_eq!(report.covered_passed_count(), 13);
    assert!(report.summary_zh().contains("13/13"));
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
    publish_script_sandbox(snapshot.clone());
    assert_eq!(last_script_sandbox().as_ref(), Some(&snapshot));
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
