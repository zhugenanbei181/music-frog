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
use infiltrator_application::snapshot_application::SnapshotApplication;
use infiltrator_composition::mihomo_configuration::profiles_for_mihomo;
use infiltrator_composition::snapshot_restore_fixture::SnapshotRestoreFixture;
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::parity::FeatureId;
use infiltrator_test_support::language_choice_fixtures::LanguageCaptureProcess;
use infiltrator_test_support::profile_store_fixtures::{
    CAPTURE_LARGE_PROFILE, CAPTURE_PROFILE, ProfileStoreCapture,
};
use infiltrator_test_support::snapshot_store_fixtures::SnapshotStoreCapture;
use std::path::PathBuf;
use std::sync::Arc;

/// The broken YAML the demo types into the document.
///
/// The line is a real YAML syntax error, so the shared codec's own
/// inspection reports the line the editor has to navigate to.
const BROKEN_YAML: &str = "  - [unterminated\n";

/// The catalogue entry the snippet gesture inserts.
///
/// A routing rule spliced under the document's own `rules:` key keeps the
/// document valid, so the shared codec really accepts the insertion.
const CAPTURE_SNIPPET_ID: &str = "rule-domain";

/// The line the snippet gesture puts the caret on.
///
/// The capture document's `rules:` key: the rule body is spliced right below
/// it, so the inserted line stays inside the document's own block sequence.
const CAPTURE_SNIPPET_LINE: usize = 3;

pub(super) fn activate(state: &mut AppState, feature: FeatureId) -> Task<Message> {
    match feature {
        FeatureId::EditorYamlErrorNavigation => activate_yaml_error(state),
        FeatureId::EditorLargeDocumentWindow => activate_large_document(state),
        FeatureId::EditorSnippetInsertion => activate_snippet_insertion(state),
        FeatureId::ProfilesDiffViewer => activate_snapshot_diff(state),
        _ => Task::none(),
    }
}

/// The user opens the capture store's large document: the editor renders the
/// owner's real bytes through the shared bounded window, so a document that is
/// far beyond one viewport is shown as a window instead of the whole file.
fn activate_large_document(state: &mut AppState) -> Task<Message> {
    attach_document_owner(state);
    let path = PathBuf::from(format!("/isolated-capture/{CAPTURE_LARGE_PROFILE}.yaml"));
    state.update(Message::EditProfile(path))
}

/// The user opens a real document and inserts a catalogue snippet at the
/// caret: the shared codec splices the bytes and validates the result.
fn activate_snippet_insertion(state: &mut AppState) -> Task<Message> {
    attach_document_owner(state);
    let path = PathBuf::from(format!("/isolated-capture/{CAPTURE_PROFILE}.yaml"));
    state.update(Message::EditProfile(path))
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
    // Opening a document also asks the owner for the profile's snapshot
    // history. The capture composes the snapshot owner, so that read really
    // succeeds: an unconfigured snapshot owner would answer with a typed
    // failure that lands in the surface's error banner and repaints the
    // frame after the capture published its readback.
    core.install_command_handler(Arc::new(
        CommandApplication::new()
            .with_profile(profiles_for_mihomo(store.clone()))
            .with_snapshots(SnapshotApplication::new(
                store,
                Arc::new(SnapshotStoreCapture::default()),
            )),
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
    } else if loaded
        && state.shell.capture_scenario == Some(FeatureId::EditorSnippetInsertion)
        && state.editor.document_session.can_edit()
    {
        // The caret walks to the document's own `rules:` key and the shared
        // catalogue entry is inserted there: the production splice path, with
        // the codec's own validation.
        let mut task = task.chain(Task::done(Message::EditorAction(Action::Move(
            Motion::DocumentStart,
        ))));
        for _ in 0..CAPTURE_SNIPPET_LINE {
            task = task.chain(Task::done(Message::EditorAction(Action::Move(
                Motion::Down,
            ))));
        }
        task.chain(Task::done(Message::EditorAction(Action::Move(Motion::End))))
            .chain(Task::done(Message::InsertYamlSnippet(CAPTURE_SNIPPET_ID)))
    } else {
        task
    }
}
