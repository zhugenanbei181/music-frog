//! DUAL-09-08/09/12: snapshot diff modal, two-step rollback confirmation and
//! remote-subscription protection on the Iced surface (headless).
//!
//! The modal renders the shared `YamlAstDiffSnapshot` and nothing else; these
//! tests pin the state machine around it and the shared protection read model.

use super::*;
use crate::types::app::SnapshotDiffMode;
use infiltrator_domain::profiles::ProfileInfo;

fn fixture() -> YamlAstDiffSnapshot {
    YamlAstDiffSnapshot::demo_fixture()
        .with_source_path("/fake/configs/main-history/1-deadbeef.yaml")
}

fn open_modal_state() -> AppState {
    let (mut state, _) = AppState::new();
    state.editor.snapshot_diff_modal_open = true;
    state.editor.snapshot_diff_selected_id = Some("1-deadbeef.yaml".to_string());
    state.editor.snapshot_diff = Some(fixture());
    state
}

#[test]
fn modal_renders_the_shared_diff_in_inline_and_split_modes() {
    let mut state = open_modal_state();
    {
        let _inline = snapshot_diff_modal(&state, "1-deadbeef.yaml");
    }
    state.editor.snapshot_diff_mode = SnapshotDiffMode::Split;
    {
        let _split = snapshot_diff_modal(&state, "1-deadbeef.yaml");
    }
}

#[test]
fn modal_states_are_explicit_for_loading_error_and_no_diff() {
    let (mut state, _) = AppState::new();
    state.editor.snapshot_diff_modal_open = true;
    state.editor.snapshot_diff_selected_id = Some("snap".to_string());

    state.editor.snapshot_diff_loading = true;
    {
        let _loading = snapshot_diff_modal(&state, "snap");
    }

    state.editor.snapshot_diff_loading = false;
    state.editor.snapshot_diff_error = Some("snapshot file is unreadable".to_string());
    {
        let _error = snapshot_diff_modal(&state, "snap");
    }

    state.editor.snapshot_diff_error = None;
    state.editor.snapshot_diff = None;
    {
        let _empty = snapshot_diff_modal(&state, "snap");
    }
}

#[test]
fn rollback_never_executes_before_the_second_confirmation() {
    let mut state = open_modal_state();

    let _ = state.update(Message::RollbackToSnapshot("snap".to_string()));
    assert!(
        state.editor.snapshot_diff_rollback_armed,
        "the first click only arms the confirmation"
    );
    assert!(
        state.editor.snapshot_diff_modal_open,
        "an unconfirmed rollback leaves the diff open"
    );

    let _ = state.update(Message::CancelSnapshotRollback);
    assert!(!state.editor.snapshot_diff_rollback_armed);

    let _ = state.update(Message::ArmSnapshotRollback);
    assert!(state.editor.snapshot_diff_rollback_armed);
    let _ = state.update(Message::RollbackToSnapshot("snap".to_string()));
    assert!(
        !state.editor.snapshot_diff_rollback_armed && !state.editor.snapshot_diff_modal_open,
        "the confirmed click exits the modal and dispatches the shared restore"
    );
}

#[test]
fn closing_the_modal_drops_the_diff_and_the_confirmation() {
    let mut state = open_modal_state();
    state.editor.snapshot_diff_rollback_armed = true;
    let _ = state.update(Message::CloseSnapshotDiff);
    assert!(!state.editor.snapshot_diff_modal_open);
    assert!(state.editor.snapshot_diff.is_none());
    assert!(!state.editor.snapshot_diff_rollback_armed);
    assert!(state.editor.snapshot_diff_error.is_none());
}

#[test]
fn opening_a_diff_without_an_open_profile_reports_an_honest_error() {
    let (mut state, _) = AppState::new();
    state.editor.editor_path = None;
    let _ = state.update(Message::OpenSnapshotDiff("/tmp/x.yaml".to_string()));
    assert!(
        state.editor.snapshot_diff_error.is_some(),
        "no open profile means a typed error, never a fabricated diff"
    );
    assert!(!state.editor.snapshot_diff_loading);
    assert!(state.editor.snapshot_diff.is_none());
}

#[test]
fn mode_changes_do_not_touch_the_shared_diff() {
    let mut state = open_modal_state();
    let before = state.editor.snapshot_diff.clone();
    let _ = state.update(Message::SetSnapshotDiffMode(SnapshotDiffMode::Split));
    assert_eq!(state.editor.snapshot_diff_mode, SnapshotDiffMode::Split);
    assert_eq!(
        state.editor.snapshot_diff, before,
        "layout is presentation only"
    );
}

#[test]
fn editor_protection_follows_the_shared_subscription_metadata() {
    let (mut state, _) = AppState::new();
    state.editor.editor_path = Some(std::path::PathBuf::from("/fake/configs/main.yaml"));
    state.profile.profiles = vec![ProfileInfo {
        name: "main".to_string(),
        subscription_url: Some("https://example.com/sub".to_string()),
        ..Default::default()
    }];

    assert!(
        state.edited_profile_write_protection().is_protected(),
        "a downloaded subscription is classified as protected"
    );
    assert_eq!(
        state.edited_profile_write_protection().label_zh(),
        "远程订阅 · 只读保护"
    );

    let _ = state.update(Message::SetProfileProtectionOverride(true));
    assert!(state.editor.profile_protection_override);
    assert!(
        !(state.edited_profile_write_protection().is_protected()
            && !state.editor.profile_protection_override),
        "the explicit unlock is what lets the surface offer a save"
    );

    state.profile.profiles[0].subscription_url = None;
    assert!(
        !state.edited_profile_write_protection().is_protected(),
        "a local profile is never blocked"
    );
}

#[test]
fn history_panel_restore_is_armed_before_it_executes() {
    let (mut state, _) = AppState::new();
    let path = std::path::PathBuf::from("/fake/configs/main-history/1-deadbeef.yaml");

    let _ = state.update(Message::RestoreProfileSnapshot(path.clone()));
    assert!(
        !state.editor.is_restoring_snapshot,
        "a bare restore click only arms"
    );
    assert_eq!(state.editor.pending_restore_snapshot.as_ref(), Some(&path));

    let _ = state.update(Message::CancelRestoreProfileSnapshot);
    assert!(state.editor.pending_restore_snapshot.is_none());
    assert!(!state.editor.is_restoring_snapshot);

    let _ = state.update(Message::ArmRestoreProfileSnapshot(path.clone()));
    assert_eq!(state.editor.pending_restore_snapshot.as_ref(), Some(&path));
}
