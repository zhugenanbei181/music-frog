//! test-intent: behavior
use crate::sync_page_model::{ConflictResolution, SyncPageModel};
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::sync::SyncStatus;
use infiltrator_contract::sync_snapshot::{SyncConflictSnapshot, SyncPageSnapshot};
use lurker_core_contract::page::PageStatus;

fn page(status: SyncStatus, conflict: Option<SyncConflictSnapshot>) -> SyncPageSnapshot {
    SyncPageSnapshot {
        status,
        server_url: "https://dav.example.com".to_owned(),
        username: "musicfrog".to_owned(),
        last_sync: Some("2026-10-20T08:00:00Z".to_owned()),
        auto_sync: true,
        conflict,
        snapshots: Vec::new(),
        history_status: PageStatus::Ready,
    }
}

fn conflict() -> SyncConflictSnapshot {
    SyncConflictSnapshot {
        remote_device: "phone".to_owned(),
        conflict_time: "2026-10-20T08:00:00Z".to_owned(),
        conflicting_keys: vec![("proxy".to_owned(), "DIRECT".to_owned(), "Proxy".to_owned())],
    }
}

#[test]
fn an_unobserved_page_reports_unknown_instead_of_disconnected() {
    let model = SyncPageModel::default();
    assert_eq!(model.status(), SyncStatus::Unknown);
    assert!(model.conflict().is_none());
    assert!(model.conflict_fields().is_empty());
    assert!(model.last_sync().is_none());
    assert_eq!(model.history_status(), PageStatus::Loading);
}

#[test]
fn the_owner_observation_is_read_without_a_second_shape() {
    let mut model = SyncPageModel::default();
    model.observe_page(page(SyncStatus::Connected, None));

    assert_eq!(model.status(), SyncStatus::Connected);
    assert_eq!(model.server_url(), "https://dav.example.com");
    assert_eq!(model.username(), "musicfrog");
    assert_eq!(model.last_sync(), Some("2026-10-20T08:00:00Z"));
    assert_eq!(model.auto_sync(), Some(true));
    assert!(!model.is_syncing());
}

#[test]
fn the_syncing_indicator_comes_from_the_owner() {
    let mut model = SyncPageModel::default();
    model.observe_page(page(SyncStatus::Syncing, None));
    assert!(model.is_syncing());

    model.observe_page(page(SyncStatus::Connected, None));
    assert!(!model.is_syncing());
}

#[test]
fn the_conflict_fields_are_one_shared_projection() {
    let mut model = SyncPageModel::default();
    model.observe_page(page(SyncStatus::Conflict, Some(conflict())));

    let fields = model.conflict_fields();
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].key, "proxy");
    assert_eq!(fields[0].local_value, "DIRECT");
    assert_eq!(fields[0].remote_value, "Proxy");
}

#[test]
fn the_resolution_is_one_typed_value() {
    let mut model = SyncPageModel::default();
    model.observe_page(page(SyncStatus::Conflict, Some(conflict())));

    assert_eq!(model.resolution(), ConflictResolution::KeepLocal);
    assert_eq!(
        model.resolve_conflict_intent(),
        Some(CommandIntent::ResolveConflictKeepLocal)
    );

    model.select_resolution(ConflictResolution::TakeRemote);
    assert_eq!(
        model.resolve_conflict_intent(),
        Some(CommandIntent::ResolveConflictTakeRemote)
    );

    model.select_resolution(ConflictResolution::MergeBoth);
    assert_eq!(
        model.resolve_conflict_intent(),
        Some(CommandIntent::ResolveConflictKeepLocal)
    );
}

#[test]
fn a_page_without_a_conflict_has_no_resolution_intent() {
    let mut model = SyncPageModel::default();
    model.observe_page(page(SyncStatus::Connected, None));
    assert!(model.resolve_conflict_intent().is_none());
}

#[test]
fn the_two_operations_have_their_own_phases() {
    let mut model = SyncPageModel::default();
    model.observe_page(page(SyncStatus::Connected, None));

    let sync = model.sync_now_phase_mut().begin();
    assert!(model.sync_now_phase().is_running());
    assert!(!model.resolve_phase().is_running());

    assert!(model.sync_now_phase_mut().succeed(sync));
    assert!(!model.sync_now_phase().is_running());
    assert_eq!(model.sync_now_intent(), CommandIntent::SyncNow);
}
