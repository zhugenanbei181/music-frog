use super::*;

#[test]
fn test_format_short_sha() {
    assert_eq!(format_short_sha("1a2b3c4d5e6f7890"), "1a2b3c4d");
    assert_eq!(format_short_sha("short"), "short");
    assert_eq!(format_short_sha(""), "");
}

#[test]
fn test_format_syntax_line_pill() {
    assert_eq!(format_syntax_line_pill(10), "Line 10");
    assert_eq!(format_syntax_line_pill(1), "Line 1");
}

#[test]
fn test_editor_view_render_all_panes() {
    {
        let (mut state, _) = AppState::new();
        state.editor.editor_pane = EditorPane::Profile;
        let _v = view(&state);
    }
    {
        let (mut state, _) = AppState::new();
        state.editor.editor_pane = EditorPane::Mixin;
        let _v = view(&state);
    }
    {
        let (mut state, _) = AppState::new();
        state.editor.editor_pane = EditorPane::Filter;
        let _v = view(&state);
    }
}

#[test]
fn test_editor_view_with_syntax_error() {
    let (mut state, _) = AppState::new();
    state.editor.syntax_error = Some("Mapping values are not allowed here".into());
    state.editor.syntax_error_line = Some(14);
    let _v = view(&state);
}

// ---- DUAL-09-05/06/07: shared formatter + shared prune view -----------------

fn history_fixture() -> infiltrator_contract::snapshot_history::SnapshotHistorySnapshot {
    use infiltrator_contract::snapshot_history::{SnapshotEntry, SnapshotHistorySnapshot};
    SnapshotHistorySnapshot {
        profile: "main".to_owned(),
        entries: vec![
            SnapshotEntry {
                id: "/fake/configs/main-history/snap-002.yaml".to_owned(),
                file_name: "1750000100000-cafebabe.yaml".to_owned(),
                timestamp_millis: 1_750_000_100_000,
                sha256: "cafebabe00112233445566778899aabbccddeeff00112233445566778899aabb"
                    .to_owned(),
                is_newest: true,
                is_duplicate: false,
            },
            SnapshotEntry {
                id: "/fake/configs/main-history/snap-001.yaml".to_owned(),
                file_name: "1750000000000-deadbeef.yaml".to_owned(),
                timestamp_millis: 1_750_000_000_000,
                sha256: "deadbeef00112233445566778899aabbccddeeff00112233445566778899aabb"
                    .to_owned(),
                is_newest: false,
                is_duplicate: true,
            },
        ],
        keep_limit: 20,
        pending_prune: 1,
        duplicate_entries: 1,
        last_prune: None,
    }
}

#[test]
fn format_yaml_editor_keeps_comments_through_the_shared_engine() {
    let (mut state, _) = AppState::new();
    // Four-space indentation with a handwritten comment: a serde re-serialize
    // would drop the comment; the shared AST formatter must keep it.
    state.editor.editor_content =
        text_editor::Content::with_text("# 手写注释\nrules:\n    - MATCH,DIRECT\n");
    let _ = state.update(Message::FormatYamlEditor);
    let text = state.editor.editor_content.text();
    assert!(text.contains("# 手写注释"), "comment kept: {text}");
    assert!(
        text.contains("\n  - MATCH,DIRECT"),
        "indentation normalized by the shared formatter: {text}"
    );
}

#[test]
fn format_yaml_editor_refuses_an_invalid_buffer_without_rewriting_it() {
    let (mut state, _) = AppState::new();
    state.editor.editor_content = text_editor::Content::with_text("mode: [\n");
    let _ = state.update(Message::FormatYamlEditor);
    assert_eq!(
        state.editor.editor_content.text(),
        "mode: [\n",
        "a refusal must keep the user's bytes"
    );
}

#[test]
fn snapshot_history_state_follows_the_shared_prune_view_and_renders_it() {
    let (mut state, _) = AppState::new();
    let _ = state.update(Message::ProfileSnapshotsLoaded(Ok(history_fixture())));
    let history = state
        .editor
        .snapshot_history
        .as_ref()
        .expect("shared history stored");
    assert_eq!(history.pending_prune, 1);
    assert_eq!(history.duplicate_entries, 1);
    assert!(!state.editor.is_loading_snapshots);

    // The retention control clamps to the shared supported range.
    let _ = state.update(Message::SetSnapshotPruneKeep(0));
    assert_eq!(state.editor.snapshot_prune_keep, 1);
    let _ = state.update(Message::SetSnapshotPruneKeep(999));
    assert_eq!(state.editor.snapshot_prune_keep, 100);

    {
        let _rendered = view(&state);
    }

    let _ = state.update(Message::ProfileSnapshotsLoaded(Err(
        infiltrator_contract::error::InfiltratorError::Config("boom".to_owned()),
    )));
    assert!(!state.editor.is_loading_snapshots);
}

#[test]
fn editor_live_preflight_reports_the_shared_diagnostic_line() {
    let (mut state, _) = AppState::new();
    state.editor.editor_content = text_editor::Content::with_text("mode: [\n");
    let _ = state.update(Message::EditorAction(text_editor::Action::Edit(
        text_editor::Edit::Paste("x".to_string().into()),
    )));
    assert!(
        state.editor.syntax_error.is_some(),
        "the shared preflight flags the unfinished flow mapping"
    );
    assert!(
        state.editor.syntax_error_line.is_some(),
        "the blamed line travels with the diagnostic for the Line pill"
    );
    {
        let _rendered = view(&state);
    }
}
