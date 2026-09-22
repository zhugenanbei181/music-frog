//! DUAL-05-15: the shared protocol-codec regression matrix must pass every
//! covered item and name every planned one honestly.

use crate::protocol_codec_matrix_application::ProtocolCodecMatrixApplication;

#[test]
fn deterministic_matrix_passes_every_covered_item_and_names_the_planned_ones() {
    let report = ProtocolCodecMatrixApplication::run_deterministic_matrix();
    assert!(
        report.all_covered_passed(),
        "failed: {:?}",
        report
            .scenarios
            .iter()
            .filter(|row| row.covered && !row.passed)
            .collect::<Vec<_>>()
    );
    assert_eq!(report.covered_count(), 12, "{:?}", report.scenarios);
    assert_eq!(report.covered_passed_count(), 12);
    assert_eq!(
        report.not_covered_ids(),
        vec!["DUAL-05-09", "DUAL-05-10", "DUAL-05-13"]
    );
    assert_eq!(report.scenarios.len(), 15);
    for row in &report.scenarios {
        assert!(!row.detail.trim().is_empty(), "{} has no detail", row.id);
        if !row.covered {
            assert!(row.detail.len() > 20, "{} needs a real reason", row.id);
        }
    }
    assert!(
        report.summary_zh().contains("12/12"),
        "{}",
        report.summary_zh()
    );
}

#[test]
fn matrix_is_deterministic_across_runs() {
    let first = ProtocolCodecMatrixApplication::run_deterministic_matrix();
    let second = ProtocolCodecMatrixApplication::run_deterministic_matrix();
    assert_eq!(first, second);
}
