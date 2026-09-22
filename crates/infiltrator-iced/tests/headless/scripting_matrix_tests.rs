//! DUAL-10-15: the shared scripting-sandbox regression matrix asserted on the
//! Iced surface. The matrix is executed by the shared application and only
//! claims the rows it actually ran.
//! test-intent: behavior

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
        vec!["DUAL-10-01", "DUAL-10-05", "DUAL-10-09", "DUAL-10-14"],
        "the honest gaps stay uncovered"
    );
    assert_eq!(report.covered_passed_count(), 11);
    assert!(report.summary_zh().contains("11/11"));
}
