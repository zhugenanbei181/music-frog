//! Behavior tests for the four capability surfaces wired from the bottom
//! layers into Iced: the mixin overlay editor, the subscription filter
//! editor, the MRS metadata scan and the sync-conflict key-level diff merge.
//!
//! Mounted via `src/test_mounts.rs` (crate root).
//! test-intent: behavior
use crate::state::AppState;
use crate::types::app::SyncConflict;
use crate::types::message::Message;
use crate::types::options::{EditorPane, FilterDraft, SyncDiffBundle, SyncDiffState};
use iced::widget::text_editor;
use std::path::PathBuf;
use std::sync::Arc;

fn temp_options_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "iced-options-{tag}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(dir.join("configs/options")).unwrap();
    dir
}

/// Drive one async sidecar write from a sync test body.
fn futures_executor_block_on<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future)
}

#[test]
fn test_mixin_pane_switch_loads_and_saves_via_options_state() {
    let (mut state, _) = AppState::new();
    // Editor bound to a profile with a stored mixin overlay.
    let dir = temp_options_dir("mixin");
    let options = infiltrator_domain::profile_options::ProfileOptions {
        mixin: infiltrator_domain::mixin::MixinConfig {
            mode: Some("global".to_string()),
            ..Default::default()
        },
        filter: None,
    };
    futures_executor_block_on(infiltrator_core::profile_options_io::save_options(
        &dir.join("configs"),
        "alpha",
        &options,
    ))
    .unwrap();

    let _ = state.update(Message::ProfileContentLoaded(Ok((
        dir.join("configs/alpha.yaml"),
        "mode: rule\n".to_string(),
    ))));
    assert_eq!(state.editor.editor_pane, EditorPane::Profile);
    let _ = state.update(Message::SetEditorPane(EditorPane::Filter));
    assert_eq!(state.editor.editor_pane, EditorPane::Filter);
    let _ = state.update(Message::SetEditorPane(EditorPane::Profile));
    assert_eq!(state.editor.editor_pane, EditorPane::Profile);

    // Opening the Mixin pane lazily loads the stored overlay into the editor.
    let _ = state.update(Message::SetEditorPane(EditorPane::Mixin));
    let _ = state.update(Message::MixinLoaded(Ok("mode: global\n".to_string())));
    assert_eq!(state.editor.mixin_loaded_for.as_deref(), Some("alpha"));
    assert_eq!(
        state.editor.mixin_content.text(),
        "mode: global\n".to_string()
    );

    // Malformed mixin YAML is rejected before any task spawns: the saving
    // flag stays off and the error is projected through the single sink.
    for ch in "mode: [broken".chars() {
        let _ = state.update(Message::MixinEditorAction(
            iced::widget::text_editor::Action::Edit(iced::widget::text_editor::Edit::Insert(ch)),
        ));
    }
    let _ = state.update(Message::SaveMixin);
    assert!(!state.editor.is_saving_mixin);
    assert!(
        state
            .shell
            .error_msg
            .as_deref()
            .unwrap_or("")
            .contains("Mixin")
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_filter_draft_updates_and_validation_gate() {
    let (mut state, _) = AppState::new();
    let _ = state.update(Message::ProfileFilterLoaded(Ok(FilterDraft {
        include: "HK, JP".into(),
        exclude: String::new(),
        exclude_types: "trojan".into(),
        renames: "香港-(\\d+) => HK-$1".into(),
        dedup_index: 3,
    })));
    assert_eq!(state.editor.filter_draft.include, "HK, JP");
    assert_eq!(state.editor.filter_draft.dedup_index, 3);

    // Free-text edits flow into the draft.
    let _ = state.update(Message::UpdateFilterExclude("剩余流量".into()));
    assert_eq!(state.editor.filter_draft.exclude, "剩余流量");

    // The draft compiles into a stored spec (comma + semicolon splitting).
    let spec = state.editor.filter_draft.to_spec().unwrap();
    assert_eq!(spec.include_keywords, vec!["HK", "JP"]);
    assert_eq!(spec.exclude_keywords, vec!["剩余流量"]);
    assert_eq!(spec.exclude_types, vec!["trojan"]);
    assert_eq!(spec.rename_rules.len(), 1);
    assert_eq!(
        spec.deduplication,
        infiltrator_domain::profile_options::FilterDedup::AppendIndex
    );

    // A malformed rename line fails compilation with an actionable message.
    let bad = FilterDraft {
        renames: "没有箭头的规则".into(),
        ..FilterDraft::default()
    };
    assert!(bad.to_spec().err().unwrap().to_string().contains("=>"));

    // The filter draft loads lazily for the profile open in the editor:
    // switching to the Filter pane after ProfileContentLoaded keys the load
    // on editor_path's stem.
    let dir2 = temp_options_dir("filter");
    futures_executor_block_on(infiltrator_core::profile_options_io::save_options(
        &dir2.join("configs"),
        "alpha",
        &infiltrator_domain::profile_options::ProfileOptions {
            mixin: Default::default(),
            filter: Some(infiltrator_domain::profile_options::FilterSpec {
                include_keywords: vec!["JP".into()],
                ..infiltrator_domain::profile_options::FilterSpec::default()
            }),
        },
    ))
    .unwrap();
    let _ = state.update(Message::ProfileContentLoaded(Ok((
        dir2.join("configs/alpha.yaml"),
        "mode: rule\n".to_string(),
    ))));
    let _ = state.update(Message::SetEditorPane(EditorPane::Filter));
    let _ = state.update(Message::ProfileFilterLoaded(Ok(FilterDraft {
        include: "JP".into(),
        ..FilterDraft::default()
    })));
    assert_eq!(state.editor.filter_loaded_for.as_deref(), Some("alpha"));
    assert_eq!(state.editor.filter_draft.include, "JP");
    let _ = std::fs::remove_dir_all(&dir2);
}

#[test]
fn test_mrs_details_projection() {
    let (mut state, _) = AppState::new();
    assert!(state.editor.mrs_details.is_empty());
    let bytes = mrs_header_bytes("geo");
    let meta = infiltrator_domain::mrs::parse_mrs_header(&bytes).unwrap();
    let detail = crate::types::options::MrsProviderDetail {
        name: "geo".into(),
        behavior: "domain".into(),
        file: None,
        metadata: Some(meta),
        errors: Vec::new(),
    };
    let _ = state.update(Message::MrsDetailsReady(Ok(vec![detail])));
    assert_eq!(state.editor.mrs_details.len(), 1);
    assert!(state.editor.mrs_details[0].summary().starts_with("MRS v1"));

    // Scan errors surface through the error sink instead of silently
    // clearing the previous details.
    let _ = state.update(Message::MrsDetailsReady(Err(
        infiltrator_contract::error::InfiltratorError::Config("扫描失败".into()),
    )));
    assert!(state.shell.error_msg.is_some());
}

fn mrs_header_bytes(description: &str) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"MRS\x01");
    bytes.push(1);
    bytes.push(0);
    bytes.extend_from_slice(&42u32.to_le_bytes());
    bytes.extend_from_slice(&2048u32.to_le_bytes());
    bytes.extend_from_slice(&(description.len() as u16).to_le_bytes());
    bytes.extend_from_slice(description.as_bytes());
    bytes
}

#[test]
fn test_sync_diff_flow_pick_merge_and_cleanup() {
    let (mut state, _) = AppState::new();
    state.profile.sync_conflicts = vec![SyncConflict {
        profile: "alpha".into(),
        remote_path: PathBuf::from("/tmp/alpha.remote-conflict.yaml"),
    }];

    // A computed diff opens the merge session with per-key picks preset to
    // keep-local.
    let bundle = SyncDiffBundle {
        profile: "alpha".into(),
        remote_path: PathBuf::from("/tmp/alpha.remote-conflict.yaml"),
        added: vec!["dns".into()],
        removed: vec!["tun".into()],
        modified: vec![("port".into(), "7890".into(), "8080".into())],
    };
    let _ = state.update(Message::SyncDiffLoaded(Ok(bundle)));
    let diff = state.profile.sync_diff.as_ref().unwrap();
    assert_eq!(diff.picks.len(), 3);
    assert!(diff.picks.values().all(|pick| !pick));

    // Per-key picks flip independently; bulk picks override everything.
    let _ = state.update(Message::PickSyncDiffKey("port".into(), true));
    assert!(state.profile.sync_diff.as_ref().unwrap().picks["port"]);
    let _ = state.update(Message::SetSyncDiffPicks(true));
    assert!(
        state
            .profile
            .sync_diff
            .as_ref()
            .unwrap()
            .picks
            .values()
            .all(|pick| *pick)
    );

    // Applying without a live runtime still clears the conflict from the
    // list on success (the manager path writes the merged document).
    let _ = state.update(Message::ApplySyncDiffMerge);
    let _ = state.update(Message::SyncDiffMerged(Ok("alpha".into())));
    assert!(state.profile.sync_diff.is_none());
    assert!(state.profile.sync_conflicts.is_empty());

    // Failures keep the session open for retry and project the error.
    state.profile.sync_diff = Some(SyncDiffState::new(SyncDiffBundle {
        profile: "beta".into(),
        remote_path: PathBuf::from("/tmp/beta.yaml"),
        added: Vec::new(),
        removed: Vec::new(),
        modified: vec![("port".into(), "1".into(), "2".into())],
    }));
    let _ = state.update(Message::SyncDiffMerged(Err(
        infiltrator_contract::error::InfiltratorError::Config("合并失败".into()),
    )));
    assert!(!state.profile.is_applying_sync_diff);
    assert!(state.profile.sync_diff.is_some());
    assert!(
        state
            .shell
            .error_msg
            .as_deref()
            .unwrap_or("")
            .contains("合并失败")
    );

    // Close discards the session without touching conflicts.
    let _ = state.update(Message::CloseSyncDiff);
    assert!(state.profile.sync_diff.is_none());
}

#[test]
fn test_editor_yaml_syntax_preflight() {
    let (mut state, _) = AppState::new();
    assert!(state.editor.syntax_error.is_none());

    // Action on valid YAML -> syntax_error is None
    let _ = state.update(Message::EditorAction(text_editor::Action::Edit(
        text_editor::Edit::Paste(Arc::new("port: 7890\nmode: rule".to_string())),
    )));
    assert!(state.editor.syntax_error.is_none());

    // Action on invalid YAML -> syntax_error is Some
    let _ = state.update(Message::EditorAction(text_editor::Action::Edit(
        text_editor::Edit::Paste(Arc::new("\nmode: [invalid".to_string())),
    )));
    assert!(state.editor.syntax_error.is_some());
    assert!(state.editor.syntax_error_line.is_some());
}
