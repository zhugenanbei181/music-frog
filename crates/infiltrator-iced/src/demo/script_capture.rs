//! The editor's script pane performs its own sandbox run and export review.
//!
//! Both gestures run the *shared* script service, so the console renders the
//! owner's real report and the review modal the owner's real bytes.
use crate::demo::owner::attach_capture_owner;
use crate::state::AppState;
use crate::types::message::Message;
use crate::types::options::EditorPane;
use crate::types::script::ScriptAction;
use iced::Task;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::script_application::ScriptApplication;
use infiltrator_application::script_export_application::ScriptExportApplication;
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::parity::FeatureId;
use infiltrator_contract::script_export::ScriptExportKind;
use infiltrator_test_support::language_choice_fixtures::LanguageCaptureProcess;
use std::sync::Arc;

/// The YAML the sandbox transforms: a real document with two nodes.
const SANDBOX_INPUT_YAML: &str =
    "proxies:\n  - name: Example-HK\n    type: ss\n    server: hk.example.com\n    port: 8443\n";

pub(super) fn activate(state: &mut AppState, feature: FeatureId) -> Task<Message> {
    match feature {
        FeatureId::ProfilesScriptSandbox => activate_sandbox(state),
        FeatureId::ProfilesScriptExportReview => activate_export_review(state),
        _ => Task::none(),
    }
}

/// The owner that runs the shared script service.
fn attach_script_owner(state: &mut AppState) {
    let core = CoreApplication::new(
        Arc::new(LanguageCaptureProcess),
        Arc::new(LanguageCaptureProcess),
        tokio_application_runtime().expect("isolated capture runtime"),
    );
    core.install_command_handler(Arc::new(
        CommandApplication::new()
            .with_scripts(ScriptApplication::new(), ScriptExportApplication::new(None)),
    ));
    attach_capture_owner(state, core);
}

/// The user opens the script pane and runs the preset script over the real
/// input document: the shared engine evaluates it and the console renders the
/// owner's report.
fn activate_sandbox(state: &mut AppState) -> Task<Message> {
    attach_script_owner(state);
    let _ = state.update(Message::SetEditorPane(EditorPane::Script));
    let _ = state.update(Message::Script(ScriptAction::EditYaml(
        SANDBOX_INPUT_YAML.to_string(),
    )));
    let preset = ScriptApplication::new()
        .builtin_presets()
        .into_iter()
        .next()
        .map(|preset| preset.id)
        .unwrap_or_default();
    let _ = state.update(Message::Script(ScriptAction::SelectPreset(preset)));
    state.update(Message::Script(ScriptAction::Run))
}

/// The user asks for the script export: the shared export service freezes the
/// real bytes and the review modal renders them.
fn activate_export_review(state: &mut AppState) -> Task<Message> {
    attach_script_owner(state);
    let _ = state.update(Message::SetEditorPane(EditorPane::Script));
    let _ = state.update(Message::Script(ScriptAction::EditYaml(
        SANDBOX_INPUT_YAML.to_string(),
    )));
    let preset = ScriptApplication::new()
        .builtin_presets()
        .into_iter()
        .next()
        .map(|preset| preset.id)
        .unwrap_or_default();
    let _ = state.update(Message::Script(ScriptAction::SelectPreset(preset)));
    state.update(Message::Script(ScriptAction::Export(
        ScriptExportKind::DirectiveDslScript,
    )))
}
