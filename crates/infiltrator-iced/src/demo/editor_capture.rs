//! The editor page performs its own document gestures against an isolated
//! owner.
//!
//! Both gestures load a *real* document from the capture store, so the editor
//! renders the owner's bytes and the shared codec's own verdict.
use crate::demo::owner::attach_capture_owner;
use crate::state::AppState;
use crate::types::message::Message;
use iced::Task;
use iced::widget::text_editor::{Action, Edit, Motion};
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_composition::mihomo_configuration::profiles_for_mihomo;
use infiltrator_composition::snapshot_restore_fixture::SnapshotRestoreFixture;
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::parity::FeatureId;
use infiltrator_test_support::language_choice_fixtures::LanguageCaptureProcess;
use infiltrator_test_support::profile_store_fixtures::{
    CAPTURE_LARGE_PROFILE, ProfileStoreCapture,
};
use std::path::PathBuf;
use std::sync::Arc;

/// The broken YAML the demo types into the document.
///
/// The line is a real YAML syntax error, so the shared codec's own
/// inspection reports the line the editor has to navigate to.
const BROKEN_YAML: &str = "  - [unterminated\n";

pub(super) fn activate(state: &mut AppState, feature: FeatureId) -> Task<Message> {
    match feature {
        FeatureId::EditorYamlErrorNavigation => activate_yaml_error(state),
        FeatureId::ProfilesDiffViewer => activate_snapshot_diff(state),
        _ => Task::none(),
    }
}

/// The user breaks the YAML of a large real document: the shared codec
/// inspects the buffer, reports the offending line and the editor navigates
/// to it.
fn activate_yaml_error(state: &mut AppState) -> Task<Message> {
    attach_document_owner(state);
    let path = PathBuf::from(format!("/isolated-capture/{CAPTURE_LARGE_PROFILE}.yaml"));
    state.update(Message::EditProfile(path))
}

/// The user opens the difference between the edited profile and its stored
/// snapshot: the shared application computes the real diff.
fn activate_snapshot_diff(state: &mut AppState) -> Task<Message> {
    let fixture = SnapshotRestoreFixture::new();
    let core = CoreApplication::new(
        Arc::new(LanguageCaptureProcess),
        Arc::new(LanguageCaptureProcess),
        tokio_application_runtime().expect("isolated capture runtime"),
    );
    core.install_command_handler(Arc::new(
        CommandApplication::new()
            .with_snapshots(fixture.application.clone())
            .with_profile(fixture.profile_application()),
    ));
    attach_capture_owner(state, core);
    // The diff is computed against the profile the editor has open.
    state.editor.editor_path = Some(PathBuf::from("/memory/configs/main.yaml"));
    state.update(Message::OpenSnapshotDiff(
        fixture.target.snapshot_id.clone(),
    ))
}

/// The owner that serves the capture store's real documents.
fn attach_document_owner(state: &mut AppState) {
    let store = Arc::new(ProfileStoreCapture::default());
    let core = CoreApplication::new(
        Arc::new(LanguageCaptureProcess),
        Arc::new(LanguageCaptureProcess),
        tokio_application_runtime().expect("isolated capture runtime"),
    );
    core.install_command_handler(Arc::new(
        CommandApplication::new().with_profile(profiles_for_mihomo(store)),
    ));
    attach_capture_owner(state, core);
}

/// The follow-up of the YAML gesture: once the document is open, the user
/// navigates to its end and types the broken line there.
pub(super) fn update(state: &mut AppState, message: Message) -> Task<Message> {
    let loaded = matches!(&message, Message::ProfileContentLoaded(_));
    let task = state.update(message);
    if loaded
        && state.shell.capture_scenario == Some(FeatureId::EditorYamlErrorNavigation)
        && state.editor.document_session.can_edit()
    {
        task.chain(Task::done(Message::EditorAction(Action::Move(
            Motion::DocumentEnd,
        ))))
        .chain(Task::done(Message::EditorAction(Action::Edit(
            Edit::Paste(Arc::new(BROKEN_YAML.to_string())),
        ))))
    } else {
        task
    }
}
