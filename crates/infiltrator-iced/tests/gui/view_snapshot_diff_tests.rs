//! DUAL-09-08/09/12: snapshot diff modal, two-step rollback confirmation and
//! remote-subscription protection on the Iced surface (headless).
//!
//! The modal renders the shared `YamlAstDiffSnapshot` and nothing else; these
//! tests pin the state machine around it and the shared protection read model.

use super::*;
use crate::state::editor::SnapshotDiffModalState;
use crate::types::app::SnapshotDiffMode;
use infiltrator_domain::profiles::ProfileInfo;
use infiltrator_presentation::profile_editor_projection::protection_label;
use infiltrator_shared::locales::Lang;
use lurker_core_contract::profile_protection::ProfileWriteAuthorization;
use std::path::PathBuf;

fn fixture() -> YamlAstDiffSnapshot {
    YamlAstDiffSnapshot::demo_fixture()
        .with_source_path("/fake/configs/main-history/1-deadbeef.yaml")
}

fn open_modal_state() -> AppState {
    let (mut state, _) = AppState::new();
    state.editor.editor_path = Some(PathBuf::from("/memory/configs/main.yaml"));
    state.editor.snapshot_diff_modal = SnapshotDiffModalState::Loaded {
        snapshot_id: "1-deadbeef.yaml".to_string(),
        diff: fixture(),
    };
    state
}
#[test]
fn rollback_never_executes_before_the_second_confirmation() {
    let mut state = open_modal_state();

    let before = state.editor.editor_content.text();
    let _ = state.update(Message::RollbackToSnapshot("snap".to_string()));
    assert!(
        state.editor.snapshot_restore.is_visible(),
        "the launcher opens an independent review surface"
    );
    assert!(
        state.editor.snapshot_diff_modal.is_open(),
        "opening review preserves the diff inspector"
    );
    assert_eq!(state.editor.editor_content.text(), before);
    assert!(state.editor.snapshot_restore.restored().is_none());
    let _ = state.update(Message::CancelRestoreProfileSnapshot);
    assert!(
        !state.editor.snapshot_restore.is_visible(),
        "failed preparation can be closed without a write"
    );
    assert_eq!(state.editor.editor_content.text(), before);
}

#[test]
fn closing_the_modal_drops_the_diff_and_the_confirmation() {
    let mut state = open_modal_state();
    let _ = state.update(Message::CloseSnapshotDiff);
    assert_eq!(state.editor.snapshot_diff_modal, SnapshotDiffModalState::Closed);
    assert!(state.editor.snapshot_diff_modal.diff().is_none());
    assert!(state.editor.snapshot_diff_modal.failure().is_none());
}

#[test]
fn opening_a_diff_without_an_open_profile_reports_an_honest_error() {
    let (mut state, _) = AppState::new();
    state.editor.editor_path = None;
    let _ = state.update(Message::OpenSnapshotDiff("/tmp/x.yaml".to_string()));
    assert!(
        state.editor.snapshot_diff_modal.failure().is_some(),
        "no open profile means a typed error, never a fabricated diff"
    );
    assert!(!state.editor.snapshot_diff_modal.is_loading());
    assert!(state.editor.snapshot_diff_modal.diff().is_none());
}

#[test]
fn mode_changes_do_not_touch_the_shared_diff() {
    let mut state = open_modal_state();
    let before = state.editor.snapshot_diff_modal.diff().cloned();
    let _ = state.update(Message::SetSnapshotDiffMode(SnapshotDiffMode::Split));
    assert_eq!(state.editor.snapshot_diff_mode, SnapshotDiffMode::Split);
    assert_eq!(
        state.editor.snapshot_diff_modal.diff().cloned(),
        before,
        "layout is presentation only"
    );
}

#[test]
fn editor_protection_follows_the_shared_subscription_metadata() {
    let (mut state, _) = AppState::new();
    state.editor.editor_path = Some(PathBuf::from("/fake/configs/main.yaml"));
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
        protection_label(state.edited_profile_write_protection(), &Lang("zh-CN")),
        "远程订阅 · 只读保护"
    );

    let _ = state.update(Message::SetProfileWriteAuthorization(
        ProfileWriteAuthorization::ExplicitUnlock,
    ));
    assert!(
        state
            .editor
            .profile_write_authorization
            .allows_protected_write()
    );
    assert!(
        !(state.edited_profile_write_protection().is_protected()
            && !state
                .editor
                .profile_write_authorization
                .allows_protected_write()),
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
    state.editor.editor_path = Some(PathBuf::from("/fake/configs/main.yaml"));
    let path = PathBuf::from("/fake/configs/main-history/1-deadbeef.yaml");
    let before = state.editor.editor_content.text();
    let _ = state.update(Message::RestoreProfileSnapshot(path.clone()));
    assert!(state.editor.snapshot_restore.is_visible());
    assert!(!state.editor.snapshot_restore.is_busy());
    assert!(state.editor.snapshot_restore.restored().is_none());
    assert_eq!(
        state
            .editor
            .snapshot_restore
            .target()
            .as_ref()
            .unwrap()
            .snapshot_id,
        path.to_string_lossy()
    );
    let _ = state.update(Message::CancelRestoreProfileSnapshot);
    assert!(!state.editor.snapshot_restore.is_visible());
    assert_eq!(state.editor.editor_content.text(), before);
}

/// DUAL-09-14: the modal can recompute the open diff from the shared snapshot
/// application — the same action the Bevy card's 「刷新差异」 offers.
#[test]
fn refresh_recomputes_the_open_diff_through_the_shared_application() {
    let (mut state, _) = AppState::new();
    let _ = state.update(Message::RefreshSnapshotDiff);
    assert!(
        !state.editor.snapshot_diff_modal.is_loading(),
        "nothing is open, so refresh is an inert no-op"
    );
    assert_eq!(state.editor.snapshot_diff_modal, SnapshotDiffModalState::Closed);

    let mut state = open_modal_state();
    state.editor.editor_path = Some(PathBuf::from("/fake/configs/main.yaml"));
    let _ = state.update(Message::RefreshSnapshotDiff);
    assert!(
        state.editor.snapshot_diff_modal.is_loading(),
        "refresh re-enters the shared loading path for the selected snapshot"
    );
    assert_eq!(
        state.editor.snapshot_diff_modal.snapshot_id(),
        Some("1-deadbeef.yaml"),
        "the refreshed target is the snapshot the modal already shows"
    );
}
