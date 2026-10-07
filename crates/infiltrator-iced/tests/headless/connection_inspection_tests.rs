//! test-intent: behavior
use crate::command_harness::recording_application;
use crate::test_support::demo_env;
use futures_util::StreamExt;
use iced_runtime::Action;
use iced_runtime::clipboard;
use iced_runtime::task::into_stream;
use infiltrator_application::connection_rate_application::project_connection;
use infiltrator_contract::capability::Availability;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::connection::timing_availability;
use infiltrator_contract::shortcuts::KeyModifiers;
use infiltrator_iced::state::AppState;
use infiltrator_iced::types::app::Route;
use infiltrator_iced::types::message::Message;
use tokio::runtime::Builder;

#[test]
fn connection_inspection_close_escape_and_navigation_do_not_execute_commands() {
    let (mut state, _) = AppState::demo(&demo_env(Route::Runtime));
    let original = state.diag.connections.clone();
    let connection = original.as_ref().unwrap().connections[0].clone();
    let (application, handler) = recording_application();
    state.commands = Some(application);
    state.shell.demo = false;
    assert_eq!(
        state
            .update(Message::InspectConnection(Some(connection.id.clone())))
            .units(),
        0
    );
    assert_eq!(
        state.diag.inspecting_connection_id.as_ref(),
        Some(&connection.id)
    );
    assert!(
        matches!(timing_availability(), Availability::Unsupported { reason } if reason.contains("DNS/TCP/TLS/TTFB"))
    );
    drop(state.view());
    assert_eq!(state.update(Message::InspectConnection(None)).units(), 0);
    let _ = state.update(Message::InspectConnection(Some(connection.id.clone())));
    assert_eq!(
        state
            .update(Message::KeyboardChord {
                key: "Escape".into(),
                modifiers: KeyModifiers::default()
            })
            .units(),
        0
    );
    assert_eq!(state.diag.inspecting_connection_id, None);
    let _ = state.update(Message::InspectConnection(Some(connection.id)));
    let _ = state.update(Message::Navigate(Route::Profiles));
    assert_eq!(state.diag.inspecting_connection_id, None);
    assert_eq!(state.diag.connections, original);
    assert!(handler.0.lock().unwrap().is_empty());
    assert_eq!(
        state
            .update(Message::InspectConnection(Some(
                "missing-connection".into()
            )))
            .units(),
        0
    );
    assert_eq!(state.diag.inspecting_connection_id, None);
    assert_eq!(state.update(Message::CopyConnectionHost).units(), 0);
}

#[test]
fn connection_copy_emits_native_write_and_disconnect_executes_shared_identity() {
    let (mut state, _) = AppState::demo(&demo_env(Route::Runtime));
    let connection = state.diag.connections.as_ref().unwrap().connections[0].clone();
    let facts = project_connection(
        &connection,
        state.diag.connection_rate_book.get(&connection.id),
    );
    let (application, handler) = recording_application();
    state.commands = Some(application);
    state.shell.demo = false;
    let _ = state.update(Message::InspectConnection(Some(connection.id.clone())));
    let copy = state.update(Message::CopyConnectionHost);
    let runtime = Builder::new_current_thread().enable_all().build().unwrap();
    runtime.block_on(async {
        let mut stream = into_stream(copy).unwrap();
        assert!(matches!(stream.next().await, Some(Action::Clipboard(clipboard::Action::Write { contents, .. })) if contents == facts.destination_host));
        assert!(stream.next().await.is_none());
    });
    assert!(handler.0.lock().unwrap().is_empty());
    let task = state.update(Message::CloseConnection(connection.id.clone()));
    assert_eq!(task.units(), 1);
    assert_eq!(state.diag.inspecting_connection_id, None);
    runtime.block_on(async {
        let mut stream = into_stream(task).unwrap();
        while let Some(action) = stream.next().await {
            if let Action::Output(message) = action {
                assert!(matches!(message, Message::OperationResult(Ok(()))));
                let _ = state.update(message);
            }
        }
    });
    assert_eq!(
        *handler.0.lock().unwrap(),
        vec![CommandIntent::CloseConnection { id: connection.id }]
    );
}

#[test]
fn connection_inspection_follows_identity_and_closes_when_the_connection_disappears() {
    let (mut state, _) = AppState::demo(&demo_env(Route::Runtime));
    let mut snapshot = state.diag.connections.clone().unwrap();
    let id = snapshot.connections[0].id.clone();
    let _ = state.update(Message::InspectConnection(Some(id.clone())));
    snapshot.connections.reverse();
    let _ = state.update(Message::ConnectionsReceived(snapshot.clone()));
    assert_eq!(state.diag.inspecting_connection_id.as_ref(), Some(&id));
    drop(state.view());
    snapshot
        .connections
        .retain(|connection| connection.id != id);
    let _ = state.update(Message::ConnectionsReceived(snapshot));
    assert_eq!(state.diag.inspecting_connection_id, None);
    assert_eq!(state.update(Message::CopyConnectionHost).units(), 0);
}
