//! DUAL-10-15: shared regression-matrix tests for the scripting sandbox.

use crate::script_sandbox_matrix_application::ScriptSandboxMatrixApplication;

#[test]
fn deterministic_matrix_passes_every_covered_item_and_names_the_planned_ones() {
    let report = ScriptSandboxMatrixApplication::run_deterministic_matrix();
    assert_eq!(report.scenarios.len(), 15);
    assert!(
        report.all_covered_passed(),
        "failed covered rows: {:?}",
        report.failed_ids()
    );
    // The three honest gaps stay uncovered with a real reason.
    assert_eq!(
        report.not_covered_ids(),
        vec!["DUAL-10-01", "DUAL-10-05", "DUAL-10-09", "DUAL-10-14"]
    );
    assert_eq!(report.covered_count(), 11);
    assert_eq!(report.covered_passed_count(), 11);
    for row in report.scenarios.iter().filter(|row| !row.covered) {
        assert!(!row.detail.trim().is_empty(), "{} lacks a reason", row.id);
    }
}

#[test]
fn matrix_is_deterministic_across_runs() {
    let first = ScriptSandboxMatrixApplication::run_deterministic_matrix();
    let second = ScriptSandboxMatrixApplication::run_deterministic_matrix();
    assert_eq!(first, second);
    assert!(first.summary_zh().contains("11/11"));
}
