use super::*;
use crate::types::app::{SyncConflict, SyncProgress};
use crate::types::options::{SyncDiffBundle, SyncDiffState};
use std::path::PathBuf;

#[test]
fn test_sync_view_idle_zh() {
    let (mut state, _) = AppState::new();
    state.shell.lang = "zh-CN".into();
    state.profile.webdav_enabled = true;
    state.profile.webdav_url = "https://dav.example.com".into();
    state.profile.webdav_user = "user".into();
    state.profile.webdav_pass = "pass".into();
    state.profile.webdav_sync_interval_mins = "30".into();
    state.profile.webdav_sync_on_startup = true;

    let _element: Element<'_, Message> = view(&state);
}

#[test]
fn test_sync_view_idle_en() {
    let (mut state, _) = AppState::new();
    state.shell.lang = "en-US".into();
    state.profile.webdav_enabled = false;

    let _element: Element<'_, Message> = view(&state);
}

#[test]
fn test_sync_view_syncing_with_progress() {
    let (mut state, _) = AppState::new();
    state.profile.is_syncing = true;
    state.profile.sync_progress = Some(SyncProgress {
        phase: "Uploading".to_string(),
        current: 2,
        total: 5,
    });

    let _element: Element<'_, Message> = view(&state);
}

#[test]
fn test_sync_view_with_conflicts_and_diff() {
    let (mut state, _) = AppState::new();
    state.profile.sync_conflicts = vec![SyncConflict {
        profile: "config-a".into(),
        remote_path: PathBuf::from("/tmp/config-a.remote.yaml"),
    }];
    state.profile.sync_diff = Some(SyncDiffState::new(SyncDiffBundle {
        profile: "config-a".into(),
        remote_path: PathBuf::from("/tmp/config-a.remote.yaml"),
        added: vec!["dns".into()],
        removed: vec![],
        modified: vec![("mode".into(), "rule".into(), "global".into())],
    }));

    let _element: Element<'_, Message> = view(&state);
}
