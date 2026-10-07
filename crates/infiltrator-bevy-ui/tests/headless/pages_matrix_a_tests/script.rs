//! Behavior cases for script.
//! test-intent: behavior

/// DUAL-10-15: the shared scripting-sandbox regression matrix asserted on the
/// Bevy surface. Same executor, same honest coverage set as Iced.
#[test]
fn test_script_sandbox_matrix_passes_on_the_bevy_surface() {
    use infiltrator_application::script_sandbox_matrix_application::ScriptSandboxMatrixApplication;
    let report = ScriptSandboxMatrixApplication::run_deterministic_matrix();
    assert_eq!(report.scenarios.len(), 15);
    assert!(
        report.all_covered_passed(),
        "failed covered rows: {:?}",
        report.failed_ids()
    );
    // The default configuration compiles and runs the real boa_engine adapter
    // through the shared application, so every row is covered: 15/15.
    assert!(report.not_covered_ids().is_empty());
    assert_eq!(report.covered_passed_count(), 15);
}
