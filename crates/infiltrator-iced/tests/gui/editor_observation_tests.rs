//! Actual command replies and independent reader facts preserve the native TEA editor.
//! test-intent: behavior
use infiltrator_composition::mihomo_configuration::profiles_for_mihomo;
use infiltrator_presentation::profile_edit_session::ProfileEditOperationPhase;
use infiltrator_presentation::profile_edit_status_projection::status;
use crate::state::AppState;
use crate::test_mounts::command_harness::{
    attach_product_application_with_accepted_snapshot, core_test_command_application,
};
use crate::test_mounts::identified_core_application::identified_core_application;
use crate::types::message::Message;
use futures_util::StreamExt;
use iced::Task;
use iced_runtime::{Action, task::into_stream};
use infiltrator_composition::surface_reader::ProductSurfaceReader;
use infiltrator_contract::core_capabilities::{CoreOperation, ProfileOperation};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::PageStatus;
use infiltrator_ports::surface::SurfaceReader;
use infiltrator_test_support::filter_capture_store::{FILTER_PROFILE, FilterCaptureStore};
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
    let profiles =
        profiles_for_mihomo(store.clone());
    let core = identified_core_application();
    core.install_command_handler(Arc::new(
        core_test_command_application([CoreOperation::Profile(ProfileOperation::Apply)])
            .with_profile(profiles.clone()),
    ));
    let reader = ProductSurfaceReader::new(
        Arc::new(core.clone()),
        SurfaceKind::IcedDesktop,
        HostKind::Desktop,
    )
    .with_profiles(profiles);
    let (mut state, _) = AppState::new();
    let initial = run(reader.read()).unwrap();
    attach_product_application_with_accepted_snapshot(&mut state, core, initial);
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
        matches!(
            &state.editor.document_session.operation,
            ProfileEditOperationPhase::Ready
        ),
        "read failure remains separate from the save operation phase: {:?}",
        state.editor.document_session.operation
    );
    assert!(
        status(
            &state.editor.document_session.status_fact(&initial),
            "en-US"
        )
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
    assert!(
        matches!(
            &state.editor.document_session.operation,
            ProfileEditOperationPhase::Failed(failure)
                if failure.code == ErrorCode::NotReady
        ),
        "the blocked save remains a typed NotReady result after read recovery: {:?}",
        state.editor.document_session.operation
    );
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
}
