//! test-intent: behavior
use crate::doctor_actions::DoctorActionFinish;
use crate::doctor_page_model::{DoctorOverallHealth, DoctorPageModel};
use infiltrator_contract::doctor::{DoctorAction, DoctorStatus};
use infiltrator_contract::surface_snapshot::DoctorCheckSnapshot;
use infiltrator_contract::surface_snapshot::DoctorPageSnapshot;
use lurker_core_contract::error::{ErrorCode, Failure};

fn page(healthy: bool) -> DoctorPageSnapshot {
    DoctorPageSnapshot {
        overall_healthy: healthy,
        last_run: "2026-10-20T00:00:00Z".to_owned(),
        checks: vec![DoctorCheckSnapshot {
            kind: None,
            detail_copy_key: None,
            id: "tun".to_owned(),
            name: "TUN".to_owned(),
            category: "network".to_owned(),
            state: DoctorStatus::Pass,
            detail: "ok".to_owned(),
            fix_available: false,
            hint: None,
        }],
        report_started_at: None,
        report_finished_at: None,
    }
}

#[test]
fn the_panel_runs_one_diagnostic_operation_at_a_time() {
    let mut model = DoctorPageModel::default();
    assert!(!model.is_busy());
    assert!(model.failure().is_none());
    assert!(model.current_action().is_none());

    let pending = model
        .begin(DoctorAction::Diagnose)
        .expect("the first diagnostic starts");
    assert!(model.is_busy());
    assert_eq!(model.current_action(), Some(&DoctorAction::Diagnose));

    let refused = model
        .begin(DoctorAction::RepairAll)
        .expect_err("a second diagnostic is refused while one is pending");
    assert_eq!(refused.code, ErrorCode::InvalidState);
    // The refused submission did not move the running one.
    assert_eq!(model.current_action(), Some(&DoctorAction::Diagnose));
    assert!(model.is_busy());

    assert_eq!(
        model.finish(pending.token, Ok(())),
        DoctorActionFinish::Applied
    );
    assert!(!model.is_busy());
}

#[test]
fn a_failed_diagnostic_keeps_the_failure_and_the_retry_action() {
    let mut model = DoctorPageModel::default();
    let pending = model
        .begin(DoctorAction::RepairOne("tun".to_owned()))
        .expect("the diagnostic starts");

    let failure = Failure::new(ErrorCode::Permission, "the host denied the repair", true);
    assert_eq!(
        model.finish(pending.token, Err(failure)),
        DoctorActionFinish::Applied
    );

    assert!(!model.is_busy());
    assert_eq!(
        model.failure().map(|failure| failure.message.as_str()),
        Some("the host denied the repair")
    );
    assert_eq!(
        model.current_action(),
        Some(&DoctorAction::RepairOne("tun".to_owned()))
    );
    assert!(model.can_retry());

    let retry = model.begin_retry().expect("the failure is retryable");
    assert!(model.is_busy());
    assert_eq!(
        model.current_action(),
        Some(&DoctorAction::RepairOne("tun".to_owned()))
    );
    assert_eq!(retry.token, pending.token + 1);
}

#[test]
fn a_stale_answer_never_moves_the_panel() {
    let mut model = DoctorPageModel::default();
    let pending = model
        .begin(DoctorAction::Diagnose)
        .expect("the diagnostic starts");

    assert_eq!(
        model.finish(pending.token + 100, Err(Failure::unsupported("late"))),
        DoctorActionFinish::Stale
    );
    assert!(model.is_busy());
    assert_eq!(model.current_action(), Some(&DoctorAction::Diagnose));
    assert!(model.failure().is_none());
}

#[test]
fn the_overall_health_is_unobserved_until_the_owner_publishes_a_report() {
    let mut model = DoctorPageModel::default();
    assert_eq!(model.overall_health(), DoctorOverallHealth::Unobserved);
    assert!(model.checks().is_empty());

    model.observe_page(page(true));
    assert_eq!(model.overall_health(), DoctorOverallHealth::Healthy);
    assert_eq!(model.checks().len(), 1);

    model.observe_page(page(false));
    assert_eq!(model.overall_health(), DoctorOverallHealth::Unhealthy);
}
