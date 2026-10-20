//! SLINT-001.9: the interaction paths of the diagnostic, synchronisation and
//! routing businesses.
//!
//! Every case drives the production Slint callbacks through the headless
//! surface harness, hands the owner's typed terminal to the shared reducer
//! that owns the command, and asserts the typed outcome the reducer keeps.

#[path = "workbench_paths/fixture.rs"]
mod fixture;

use fixture::{
    local, refusal, refuse, rejection, single_command, unsupported, workbench_snapshot,
    workbench_surface,
};
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::snapshot_restore::{
    SnapshotRestoreIdentity, SnapshotRestoreReview, SnapshotRestoreTarget,
};
use infiltrator_contract::yaml_ast_diff::YamlAstDiffSnapshot;
use infiltrator_domain::profile_source::hash_document_bytes;
use infiltrator_presentation::doctor_actions::DoctorActionPhase;
use infiltrator_presentation::snapshot_restore_workbench::SnapshotRestorePhase;
use infiltrator_slint::surface::action::workbench;
use infiltrator_slint::surface::harness::SurfaceHarness;
use lurker_core_contract::error::ErrorCode;
use lurker_core_contract::page::PageData;

// -- doctor-failure-recovery / lifecycle-recovery-panel -------------------

#[test]
fn the_diagnostic_refusal_reaches_the_panel_and_offers_the_retry() {
    let surface = workbench_surface("Doctor");
    single_command(surface.workbench_action(workbench::DOCTOR_DIAGNOSE, ""));

    refuse(
        &surface,
        &refusal("the diagnostic report could not be produced"),
    );

    let doctor = surface
        .workbenches()
        .doctor
        .lock()
        .expect("the doctor reducer");
    assert!(matches!(doctor.phase, DoctorActionPhase::Failed { .. }));
    drop(doctor);

    let command = single_command(surface.workbench_action(workbench::DOCTOR_RETRY, ""));
    assert_eq!(command, CommandIntent::RunDoctorDiagnostics);
}

#[test]
fn the_diagnostic_retry_keeps_the_refused_repair() {
    let surface = workbench_surface("Doctor");
    single_command(surface.workbench_action(workbench::DOCTOR_REPAIR, "capture.configuration"));

    refuse(&surface, &refusal("the repair was refused"));

    let command = single_command(surface.workbench_action(workbench::DOCTOR_RETRY, ""));
    assert_eq!(
        command,
        CommandIntent::RepairDoctorIssue {
            check_id: "capture.configuration".to_owned()
        }
    );
}

// -- sync-conflict-resolver ----------------------------------------------

#[test]
fn the_sync_conflict_is_deferred_without_the_owner() {
    let surface = workbench_surface("Sync");
    assert!(
        surface.workbench_item("sync-conflict-detail").is_some(),
        "the published conflict is shown"
    );

    local(surface.workbench_action(workbench::SYNC_CONFLICT_CANCEL, ""));

    assert!(surface.workbench_item("sync-conflict-detail").is_none());
}

#[test]
fn the_sync_conflict_refusal_reaches_the_notice_and_keeps_the_conflict() {
    let surface = workbench_surface("Sync");
    single_command(surface.workbench_action(workbench::SYNC_KEEP_LOCAL, ""));

    refuse(&surface, &refusal("the remote refused the resolution"));

    assert_eq!(surface.notice(), "the remote refused the resolution");
    assert!(surface.workbench_item("sync-conflict-detail").is_some());
}

// -- sync-restore-confirm / profiles-snapshot-restore-confirm ------------

#[test]
fn the_synced_snapshot_restoration_is_prepared_through_the_shared_command() {
    let surface = workbench_surface("Sync");

    let command = single_command(surface.workbench_action(workbench::SYNC_RESTORE, "snap-1"));

    let CommandIntent::PrepareSnapshotRestore { target } = command else {
        panic!("expected the shared snapshot restoration command");
    };
    assert_eq!(target.profile, "main");
    assert_eq!(target.snapshot_id, "snap-1");
}

#[test]
fn the_synced_snapshot_restoration_review_is_cancelled_without_writing() {
    let surface = workbench_surface("Sync");
    single_command(surface.workbench_action(workbench::SYNC_RESTORE, "snap-1"));
    surface.publish_owner_output(restore_prepared());

    let command = single_command(surface.workbench_action(workbench::RESTORE_CANCEL, ""));

    let CommandIntent::CancelSnapshotRestore { identity } = command else {
        panic!("expected the shared snapshot restoration command");
    };
    assert_eq!(identity.owner, 1);
}

#[test]
fn the_snapshot_restoration_review_is_confirmed_through_the_shared_command() {
    let surface = workbench_surface("Editor");
    single_command(surface.workbench_action(workbench::RESTORE_PREPARE, ""));
    surface.publish_owner_output(restore_prepared());

    let command = single_command(surface.workbench_action(workbench::RESTORE_CONFIRM, ""));

    let CommandIntent::ConfirmSnapshotRestore { identity } = command else {
        panic!("expected the shared snapshot restoration command");
    };
    assert_eq!(identity.owner, 1);
}

#[test]
fn the_snapshot_restoration_refusal_reaches_the_panel() {
    let surface = workbench_surface("Editor");
    single_command(surface.workbench_action(workbench::RESTORE_PREPARE, ""));

    refuse(&surface, &refusal("the stored snapshot could not be read"));

    let restore = surface
        .workbenches()
        .restore
        .lock()
        .expect("the restore reducer");
    assert!(matches!(
        restore.phase(),
        SnapshotRestorePhase::PrepareFailed { .. }
    ));
    assert!(restore.failure().is_some());
}

// -- app-routing-policy-editor --------------------------------------------

#[test]
fn the_routing_rule_draft_is_dropped_without_the_owner() {
    let surface = workbench_surface("AppRouting");
    surface.set_workbench_draft_2("block");

    local(surface.workbench_action(workbench::APP_ROUTING_CANCEL, ""));

    assert_eq!(surface.workbench_draft_2(), "");
}

#[test]
fn the_routing_policy_asks_the_owner_for_a_fresh_read() {
    let surface = workbench_surface("AppRouting");

    local(surface.workbench_action(workbench::APP_ROUTING_REFRESH, ""));

    assert_eq!(surface.read_requests(), 1);
}

#[test]
fn the_routing_policy_is_refused_when_the_host_cannot_route_applications() {
    let mut snapshot = workbench_snapshot();
    snapshot.pages.app_routing =
        PageData::unavailable(unsupported("this host cannot route applications"));
    let surface = SurfaceHarness::new(Some(snapshot)).expect("headless surface");
    surface.open_page("AppRouting");

    let failure =
        rejection(surface.workbench_action(workbench::APP_ROUTING_RULE, "com.example.app"));

    assert_eq!(failure.code, ErrorCode::Unsupported);
}

/// The restoration review the owner publishes for the prepared snapshot.
fn restore_prepared() -> CommandOutput {
    let before = "proxies: []\n";
    let restored = "proxies:\n  - name: restored\n";
    let revision = hash_document_bytes(before);
    let snapshot_hash = hash_document_bytes(restored);
    let identity = SnapshotRestoreIdentity {
        owner: 1,
        sequence: 1,
        snapshot_hash,
        workspace_identity: fixture::workspace_identity_with_revision("main", &revision),
    };
    let mut difference = YamlAstDiffSnapshot::demo_fixture();
    difference.target_id = "main".to_owned();
    difference.source_path = Some("snap-1".to_owned());
    let review = SnapshotRestoreReview {
        identity,
        target: SnapshotRestoreTarget {
            profile: "main".to_owned(),
            snapshot_id: "snap-1".to_owned(),
        },
        before_yaml: before.to_owned(),
        restored_yaml: restored.to_owned(),
        difference,
    };
    CommandOutput::SnapshotRestorePrepared(Box::new(review))
}
