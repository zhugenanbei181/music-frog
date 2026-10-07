//! test-intent: behavior
//! Native TEA commands run the actual product facade, never an in-UI engine.
use crate::state::AppState;
use crate::test_mounts::command_harness::recording_application;
use crate::types::message::Message;
use crate::types::script::ScriptAction;
use futures_util::StreamExt;
use iced::Task;
use iced_runtime::{Action, task::into_stream};
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::script_application::ScriptApplication;
use infiltrator_application::script_export_application::ScriptExportApplication;
use infiltrator_contract::error::ErrorCode;
use infiltrator_contract::script_export::ScriptExportKind;
use std::sync::Arc;
use tokio::runtime::Builder;

pub(crate) fn setup(scripts: ScriptApplication, exports: ScriptExportApplication) -> AppState {
    let (commands, _) = recording_application();
    commands.install_command_handler(Arc::new(
        CommandApplication::new().with_scripts(scripts, exports),
    ));
    let (mut state, _) = AppState::new();
    state.commands = Some(commands);
    state.shell.demo = false;
    state
}
pub(crate) fn complete(state: &mut AppState, task: Task<Message>) {
    let message = Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let mut stream = into_stream(task).expect("actual script command task");
            let Some(Action::Output(message)) = stream.next().await else {
                panic!("command terminal")
            };
            assert!(stream.next().await.is_none());
            message
        });
    assert_eq!(state.update(message).units(), 0);
}
#[test]
fn native_sandbox_pending_edit_fence_clear_and_hostless_export_remain_actual_commands() {
    let scripts = ScriptApplication::new();
    let mut state = setup(
        scripts.clone(),
        ScriptExportApplication::without_host_port(),
    );
    let _ = state.update(Message::Script(ScriptAction::EditCode(
        "function main(config) { return config; }".into(),
    )));
    let _ = state.update(Message::Script(ScriptAction::EditYaml(
        "mode: rule\n".into(),
    )));
    let task = state.update(Message::Script(ScriptAction::Run));
    assert!(state.editor.script_sandbox.is_running());
    assert_eq!(state.update(Message::Script(ScriptAction::Run)).units(), 0);
    let _ = state.update(Message::Script(ScriptAction::EditYaml(
        "mode: global\n".into(),
    )));
    assert_eq!(state.editor.script_sandbox.input_yaml, "mode: rule\n");
    complete(&mut state, task);
    assert!(
        state
            .editor
            .script_sandbox
            .snapshot
            .as_ref()
            .unwrap()
            .is_success()
    );
    assert_eq!(
        scripts.observation().result.unwrap().snapshot.input_yaml,
        "mode: rule\n"
    );
    let task = state.update(Message::Script(ScriptAction::Clear));
    complete(&mut state, task);
    assert!(scripts.observation().result.is_none());
    assert!(state.editor.script_sandbox.snapshot.is_none());
    let task = state.update(Message::Script(ScriptAction::Export(
        ScriptExportKind::DirectiveDslScript,
    )));
    complete(&mut state, task);
    assert!(state.editor.script_sandbox.export_visible);
    assert!(state.editor.script_sandbox.review.is_some());
    let task = state.update(Message::Script(ScriptAction::ConfirmExport));
    complete(&mut state, task);
    assert_eq!(
        state.editor.script_sandbox.failure.as_ref().unwrap().code,
        ErrorCode::Unsupported
    );
    let task = state.update(Message::Script(ScriptAction::CancelExport));
    complete(&mut state, task);
    assert!(!state.editor.script_sandbox.export_visible);
    assert_eq!(
        state
            .update(Message::Script(ScriptAction::ConfirmExport))
            .units(),
        0
    );
}
