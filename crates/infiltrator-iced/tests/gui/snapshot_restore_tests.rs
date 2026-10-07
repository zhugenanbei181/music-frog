//! test-intent: behavior
//! Native TEA executes actual reviewed commits and retains the review through real failures.
use crate::state::AppState;
use crate::test_mounts::command_harness::recording_application;
use crate::test_mounts::script_workbench_tests::complete;
use crate::types::message::Message;
use crate::types::snapshot_restore::RestoreAction;
use iced::widget::text_editor;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_composition::snapshot_restore_fixture::{AFTER, BEFORE, SnapshotRestoreFixture};
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::error::ErrorCode;
use std::path::PathBuf;
use std::sync::Arc;
fn setup() -> (AppState, SnapshotRestoreFixture) {
    let fixture = SnapshotRestoreFixture::new();
    let (core, _) = recording_application();
    core.install_command_handler(Arc::new(
        CommandApplication::new().with_snapshots(fixture.application.clone()),
    ));
    let (mut state, _) = AppState::new();
    state.commands = Some(core);
    state.editor.editor_path = Some(PathBuf::from("/memory/configs/main.yaml"));
    state.editor.editor_content = text_editor::Content::with_text(BEFORE);
    (state, fixture)
}
#[test]
fn native_restore_cancel_preserves_editor_and_locale_preserves_frozen_identity() {
    let (mut state, fixture) = setup();
    let task = state.update(Message::SnapshotRestore(RestoreAction::Open(
        fixture.target.clone(),
    )));
    assert!(state.editor.snapshot_restore.busy());
    assert_eq!(fixture.writes(), 0);
    complete(&mut state, task);
    let review = state.editor.snapshot_restore.review.clone().unwrap();
    assert_eq!(review.before_yaml, BEFORE);
    assert_eq!(review.restored_yaml, AFTER);
    state.shell.lang = "en-US".into();
    assert_eq!(
        state.editor.snapshot_restore.review.as_ref().unwrap(),
        &review
    );
    let before = state.editor.editor_content.text();
    let _ = state.update(Message::EditorAction(text_editor::Action::Edit(
        text_editor::Edit::Insert('x'),
    )));
    assert_eq!(
        state.editor.editor_content.text(),
        before,
        "background input is fenced while review is visible"
    );
    let task = state.update(Message::SnapshotRestore(RestoreAction::Cancel));
    complete(&mut state, task);
    assert!(!state.editor.snapshot_restore.visible);
    assert_eq!(fixture.writes(), 0);
    assert_eq!(fixture.content(), BEFORE);
    assert_eq!(
        state
            .update(Message::SnapshotRestore(RestoreAction::Confirm))
            .units(),
        0
    );
}
#[test]
fn native_restore_permission_retry_and_source_change_use_real_typed_terminals() {
    let (mut state, fixture) = setup();
    let task = state.update(Message::SnapshotRestore(RestoreAction::Open(
        fixture.target.clone(),
    )));
    complete(&mut state, task);
    fixture.deny(true);
    let task = state.update(Message::SnapshotRestore(RestoreAction::Confirm));
    complete(&mut state, task);
    assert_eq!(
        state.editor.snapshot_restore.failure.as_ref().unwrap().code,
        ErrorCode::Permission
    );
    assert_eq!(fixture.writes(), 0);
    fixture.deny(false);
    let task = state.update(Message::SnapshotRestore(RestoreAction::Retry));
    complete(&mut state, task);
    assert!(state.editor.snapshot_restore.restored.is_some());
    assert_eq!(fixture.writes(), 1);
    assert_eq!(fixture.content(), AFTER);
    assert_eq!(
        state
            .update(Message::SnapshotRestore(RestoreAction::Confirm))
            .units(),
        0
    );
    assert_eq!(fixture.writes(), 1);
    let _ = state.update(Message::SnapshotRestore(RestoreAction::Cancel));
    let task = state.update(Message::SnapshotRestore(RestoreAction::Open(
        fixture.target.clone(),
    )));
    complete(&mut state, task);
    fixture.replace_source("# later change\nmode: direct\n");
    let task = state.update(Message::SnapshotRestore(RestoreAction::Confirm));
    complete(&mut state, task);
    assert_eq!(
        state.editor.snapshot_restore.failure.as_ref().unwrap().code,
        ErrorCode::NotReady
    );
    assert_eq!(fixture.writes(), 1);
    let task = state.update(Message::SnapshotRestore(RestoreAction::Cancel));
    complete(&mut state, task);
    assert!(!state.editor.snapshot_restore.visible);
    let task = state.update(Message::SnapshotRestore(RestoreAction::Open(
        fixture.target.clone(),
    )));
    complete(&mut state, task);
    fixture.replace_options(Some("# externally added sidecar\n{}\n"));
    let task = state.update(Message::SnapshotRestore(RestoreAction::Confirm));
    complete(&mut state, task);
    assert_eq!(
        state.editor.snapshot_restore.failure.as_ref().unwrap().code,
        ErrorCode::NotReady
    );
    assert_eq!(fixture.writes(), 1);
    let task = state.update(Message::SnapshotRestore(RestoreAction::Cancel));
    complete(&mut state, task);
}
#[test]
fn native_restore_rejects_uncorrelated_and_unit_completions_without_fabricating_success() {
    let (mut state, fixture) = setup();
    let task = state.update(Message::SnapshotRestore(RestoreAction::Open(
        fixture.target.clone(),
    )));
    let operation = state
        .editor
        .snapshot_restore
        .pending
        .as_ref()
        .unwrap()
        .operation;
    let _ = state.update(Message::SnapshotRestore(RestoreAction::Finished {
        operation: operation + 1,
        result: Box::new(Ok(CommandOutput::Unit)),
    }));
    assert!(state.editor.snapshot_restore.busy());
    complete(&mut state, task);
    let _task = state.update(Message::SnapshotRestore(RestoreAction::Confirm));
    let operation = state
        .editor
        .snapshot_restore
        .pending
        .as_ref()
        .unwrap()
        .operation;
    let _ = state.update(Message::SnapshotRestore(RestoreAction::Finished {
        operation,
        result: Box::new(Ok(CommandOutput::Unit)),
    }));
    assert_eq!(
        state.editor.snapshot_restore.failure.as_ref().unwrap().code,
        ErrorCode::InvalidState
    );
    assert!(state.editor.snapshot_restore.restored.is_none());
    assert_eq!(fixture.writes(), 0);
}
