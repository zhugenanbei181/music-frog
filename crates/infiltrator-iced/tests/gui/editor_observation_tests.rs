//! Actual command replies and independent reader facts preserve the native TEA editor.
//! test-intent: behavior
use crate::state::AppState;
use crate::types::message::Message;
use futures_util::StreamExt;
use iced::Task;
use iced_runtime::{Action, task::into_stream};
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::filter_capture_store::{FILTER_PROFILE, FilterCaptureStore};
use infiltrator_application::language_choice_fixtures::LanguageCaptureProcess;
use infiltrator_application::profile_application::ProfileApplication;
use infiltrator_application::surface_reader::ApplicationSurfaceReader;
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::PageStatus;
use infiltrator_ports::surface::SurfaceReader;
use std::future::Future;
use std::path::PathBuf;
use std::sync::{Arc, atomic::Ordering};
use tokio::runtime::Builder;
fn run<F: Future>(future: F) -> F::Output {
    Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future)
}
fn terminal(task: Task<Message>) -> Message {
    run(async {
        let mut stream = into_stream(task).unwrap();
        let Some(Action::Output(message)) = stream.next().await else {
            panic!("actual command reply required")
        };
        assert!(stream.next().await.is_none());
        message
    })
}
#[test]
fn tea_read_failure_retains_actual_document_keeps_typed_error_and_recovers_without_an_invented_save()
 {
    let store = Arc::new(FilterCaptureStore::default());
    let profiles = ProfileApplication::new(store.clone());
    let core = CoreApplication::new(
        Arc::new(LanguageCaptureProcess),
        Arc::new(LanguageCaptureProcess),
        tokio_application_runtime().unwrap(),
    );
    core.install_command_handler(Arc::new(
        CommandApplication::new().with_profile(profiles.clone()),
    ));
    let reader = ApplicationSurfaceReader::new(
        Arc::new(core.clone()),
        SurfaceKind::IcedDesktop,
        HostKind::Desktop,
    )
    .with_profiles(profiles);
    let (mut state, _) = AppState::new();
    state.commands = Some(core);
    let path = PathBuf::from(format!("/isolated/{FILTER_PROFILE}.yaml"));
    let task = state.update(Message::EditProfile(path.clone()));
    let reply = terminal(task);
    let _ = state.update(reply);
    let initial = state.editor.editor_content.text();
    assert!(state.editor.document_session.can_save());
    store.deny_read.store(true, Ordering::SeqCst);
    let task = state.update(Message::EditProfile(path.clone()));
    let reply = terminal(task);
    let _ = state.update(reply);
    assert_eq!(state.editor.editor_content.text(), initial);
    assert!(!state.editor.document_session.can_save());
    assert!(
        state.editor.document_session.failure.is_none(),
        "read failure is distinct from save failure"
    );
    assert!(
        state
            .editor
            .document_session
            .status(&initial, "en-US")
            .starts_with("Editor read failed:")
    );
    let task = state.update(Message::SaveProfile);
    assert_eq!(task.units(), 0);
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
    store.deny_read.store(false, Ordering::SeqCst);
    let task = state.update(Message::EditProfile(path));
    let reply = terminal(task);
    let _ = state.update(reply);
    let mut snapshot = run(reader.read()).unwrap();
    // The profile-list failure cannot hide independently verified editor observations.
    snapshot.pages.profiles.status = PageStatus::Failed {
        failure: Failure::new(ErrorCode::Storage, "list unavailable", true),
    };
    snapshot.pages.profiles.data = None;
    let _ = state.update(Message::SurfaceSnapshotUpdated(Box::new(snapshot)));
    assert!(state.editor.document_session.can_save());
    assert_eq!(state.editor.editor_content.text(), initial);
    assert!(!state.editor.document_session.saved);
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
}
