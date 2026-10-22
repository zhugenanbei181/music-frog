//! The repair progress of the diagnostics panel of the real Slint surface.
//!
//! The repair business is a state machine the shared owner drives: the panel
//! that owns it shows the pending phase while the owner still owes its
//! terminal, the owner's success closes the business, the owner's refusal
//! opens the recovery path, and the retry repeats the refused operation.
//! Every case below drives the production Slint callbacks through the
//! headless surface harness, so a surface that lost one of those phases
//! fails here.

use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::{DoctorPageSnapshot, SurfaceSnapshot};
use infiltrator_slint::SlintAction;
use infiltrator_slint::surface::action::workbench;
use infiltrator_slint::surface::harness;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::page::PageData;
use serde_json::json;

/// The check the fixture published as repairable.
const REPAIRABLE_CHECK: &str = "capture.configuration";

/// The owner snapshot the diagnostics page observes.
fn doctor_snapshot() -> SurfaceSnapshot {
    let mut snapshot = SurfaceSnapshot::unavailable(
        SurfaceKind::SlintDesktop,
        HostKind::Desktop,
        Failure::new(ErrorCode::NotReady, "initial", true),
    );
    let doctor: DoctorPageSnapshot = serde_json::from_value(json!({
        "overall_healthy": false,
        "last_run": "2026-01-01T00:00:00Z",
        "checks": [{
            "id": REPAIRABLE_CHECK,
            "name": "Configuration",
            "category": "config",
            "state": "fail",
            "detail": "configuration is invalid",
            "fix_available": true
        }],
        "report_started_at": 1,
        "report_finished_at": 2
    }))
    .expect("doctor page JSON");
    snapshot.pages.doctor = PageData::ready(doctor);
    snapshot
}

/// The headless surface with the diagnostics page open.
fn doctor_surface() -> harness::SurfaceHarness {
    let snapshot = doctor_snapshot();
    let surface = harness::SurfaceHarness::new(Some(snapshot.clone())).expect("headless surface");
    surface.publish_snapshot(snapshot);
    surface.open_page("Doctor");
    surface
}

/// The owner commands the surface dispatched.
fn dispatched_commands(outcome: harness::SurfaceOutcome) -> Vec<CommandIntent> {
    match outcome {
        harness::SurfaceOutcome::Dispatched(actions) => actions
            .into_iter()
            .map(|action| match action {
                SlintAction::Command(intent) => intent,
                other => panic!("expected an owner command, got {other:?}"),
            })
            .collect(),
        harness::SurfaceOutcome::Rejected { failure, .. } => {
            panic!("expected an owner command, got the failure {failure:?}")
        }
    }
}

/// The feedback the diagnostics panel shows for its own action.
fn feedback(surface: &harness::SurfaceHarness) -> Option<String> {
    surface
        .workbench_item("doctor-failure")
        .map(|item| item.detail.as_str().to_owned())
}

#[test]
fn the_repair_progress_is_pending_while_the_owner_owes_its_terminal() {
    let surface = doctor_surface();

    let commands =
        dispatched_commands(surface.workbench_action(workbench::DOCTOR_REPAIR, REPAIRABLE_CHECK));

    assert_eq!(
        commands,
        vec![CommandIntent::RepairDoctorIssue {
            check_id: REPAIRABLE_CHECK.to_owned(),
        }]
    );
    let pending = feedback(&surface).expect("the panel shows the running operation");
    assert!(
        !pending.is_empty(),
        "the pending phase has to name the running operation"
    );
    let diagnose = surface
        .workbench_item("doctor-diagnose")
        .expect("the diagnostic action is projected");
    assert!(
        !diagnose.enabled,
        "a running operation owns the panel, so the diagnostic action waits"
    );
}

#[test]
fn the_owner_success_closes_the_repair_progress() {
    let surface = doctor_surface();
    dispatched_commands(surface.workbench_action(workbench::DOCTOR_REPAIR, REPAIRABLE_CHECK));

    surface.publish_owner_success();

    assert_eq!(
        feedback(&surface),
        None,
        "the owner's success closes the business, so the panel shows nothing"
    );
    let diagnose = surface
        .workbench_item("doctor-diagnose")
        .expect("the diagnostic action is projected");
    assert!(
        diagnose.enabled,
        "the owner answered, so the panel accepts the next diagnostic run"
    );
}

#[test]
fn the_owner_refusal_opens_the_recovery_path_of_the_repair() {
    let surface = doctor_surface();
    dispatched_commands(surface.workbench_action(workbench::DOCTOR_REPAIR, REPAIRABLE_CHECK));

    surface.publish_owner_failure(&Failure::new(
        ErrorCode::Internal,
        "the core refused the repair",
        true,
    ));

    assert_eq!(
        feedback(&surface).as_deref(),
        Some("the core refused the repair"),
        "the panel that owns the business shows the owner's refusal"
    );
    let retry = surface
        .workbench_item("doctor-retry")
        .expect("the recovery action is projected");
    assert!(retry.enabled, "a retryable refusal opens the recovery path");
}

#[test]
fn the_retry_repeats_the_refused_repair() {
    let surface = doctor_surface();
    dispatched_commands(surface.workbench_action(workbench::DOCTOR_REPAIR, REPAIRABLE_CHECK));
    surface.publish_owner_failure(&Failure::new(
        ErrorCode::Internal,
        "the core refused the repair",
        true,
    ));

    let commands = dispatched_commands(surface.workbench_action(workbench::DOCTOR_RETRY, ""));

    assert_eq!(
        commands,
        vec![CommandIntent::RepairDoctorIssue {
            check_id: REPAIRABLE_CHECK.to_owned(),
        }],
        "the retry repeats the refused operation instead of a new one"
    );
    let pending = feedback(&surface).expect("the panel shows the running operation");
    assert!(
        !pending.is_empty(),
        "the retried operation shows its own pending phase"
    );
}
