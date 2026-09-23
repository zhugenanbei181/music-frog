//! DUAL-10-15: shared regression-matrix tests for the scripting sandbox.

use crate::script_sandbox_matrix_application::ScriptSandboxMatrixApplication;

#[test]
fn deterministic_matrix_passes_every_covered_item_with_no_gaps() {
    let report = ScriptSandboxMatrixApplication::run_deterministic_matrix();
    assert_eq!(report.scenarios.len(), 15);
    assert!(
        report.all_covered_passed(),
        "failed covered rows: {:?}",
        report.failed_ids()
    );
    // Default build: `script-engine-boa` is on, so the real Boa adapter runs
    // and every row is covered by genuine execution (15/15, no honest gap).
    #[cfg(feature = "script-engine-boa")]
    {
        assert!(report.not_covered_ids().is_empty());
        assert_eq!(report.covered_count(), 15);
        assert_eq!(report.covered_passed_count(), 15);
    }
    // Only an explicit `--no-default-features` build drops the adapter and
    // degrades the single engine row back to an honest `planned`.
    #[cfg(not(feature = "script-engine-boa"))]
    {
        assert_eq!(report.not_covered_ids(), vec!["DUAL-10-01"]);
        assert_eq!(report.covered_count(), 14);
        assert_eq!(report.covered_passed_count(), 14);
    }
    for row in report.scenarios.iter().filter(|row| !row.covered) {
        assert!(!row.detail.trim().is_empty(), "{} lacks a reason", row.id);
    }
}

#[test]
fn matrix_is_deterministic_across_runs() {
    let first = ScriptSandboxMatrixApplication::run_deterministic_matrix();
    let second = ScriptSandboxMatrixApplication::run_deterministic_matrix();
    assert_eq!(first, second);
    #[cfg(not(feature = "script-engine-boa"))]
    assert!(first.summary_zh().contains("14/14"));
    #[cfg(feature = "script-engine-boa")]
    assert!(first.summary_zh().contains("15/15"));
}
