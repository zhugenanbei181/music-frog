//! test-intent: behavior
use crate::command_harness::recording_application;
use crate::test_support::demo_env;
use infiltrator_application::connection_grouping_fixtures::grouping_snapshot;
use infiltrator_domain::connection_view::ConnectionGroupingMode;
use infiltrator_iced::state::AppState;
use infiltrator_iced::types::app::Route;
use infiltrator_iced::types::message::Message;

#[test]
fn grouping_updates_all_groups_and_preserves_search_without_commands() {
    let (mut state, _) = AppState::demo(&demo_env(Route::Runtime));
    let (application, handler) = recording_application();
    state.commands = Some(application);
    let mut snapshot = grouping_snapshot();
    let _ = state.update(Message::ConnectionsReceived(snapshot.clone()));
    assert_eq!(
        state
            .update(Message::SetConnectionGroupingMode(
                ConnectionGroupingMode::ByProcess
            ))
            .units(),
        0
    );
    assert_eq!(state.diag.connection_groups.rows().len(), 8);
    assert_eq!(state.diag.connection_groups.rows()[0].key, "client-7");
    let _ = state.update(Message::UpdateRuntimeConnectionFilter("CLIENT-0".into()));
    assert_eq!(state.diag.connection_groups.rows().len(), 1);
    assert_eq!(state.diag.connection_groups.rows()[0].count, 2);
    assert_eq!(
        state.diag.connection_groups.rows()[0].traffic,
        "↑ 2.00 KB / ↓ 4.00 KB"
    );
    let _ = state.update(Message::SetConnectionGroupingMode(
        ConnectionGroupingMode::ByHost,
    ));
    assert_eq!(
        state.diag.connection_groups.rows()[0].key,
        "api-0.example.org"
    );
    snapshot
        .connections
        .retain(|row| row.id == "group-duplicate");
    snapshot.connections[0].upload = 4096;
    let _ = state.update(Message::ConnectionsReceived(snapshot));
    assert_eq!(state.diag.connection_groups.rows()[0].count, 1);
    assert_eq!(
        state.diag.connection_groups.rows()[0].traffic,
        "↑ 4.00 KB / ↓ 2.00 KB"
    );
    let _ = state.update(Message::UpdateRuntimeConnectionFilter("no-match".into()));
    assert!(state.diag.connection_groups.rows().is_empty());
    drop(state.view());
    assert!(handler.0.lock().unwrap().is_empty());
}

#[test]
fn grouping_first_observation_respects_existing_search() {
    let (mut state, _) = AppState::demo(&demo_env(Route::Runtime));
    state.runtime.runtime_connection_filter = "CLIENT-0".into();
    let _ = state.update(Message::ConnectionsReceived(grouping_snapshot()));
    let _ = state.update(Message::SetConnectionGroupingMode(
        ConnectionGroupingMode::ByProcess,
    ));
    assert_eq!(state.diag.connection_groups.rows().len(), 1);
    assert_eq!(state.diag.connection_groups.rows()[0].key, "client-0");
    assert_eq!(state.diag.connection_groups.rows()[0].count, 2);
}
