//! test-intent: behavior
use crate::test_support::demo_env;
use infiltrator_domain::connection_view::ConnectionGroupingMode;
use infiltrator_iced::state::AppState;
use infiltrator_iced::types::app::Route;
use infiltrator_iced::types::message::Message;
use infiltrator_test_support::connection_grouping_fixtures::grouping_snapshot;

#[test]
fn grouping_updates_all_groups_and_preserves_search_without_commands() {
    let (mut state, _) = AppState::demo(&demo_env(Route::Runtime));
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
    assert_eq!(state.diag.connections_model.rows().len(), 8);
    assert_eq!(state.diag.connections_model.rows()[0].key, "client-7");
    let _ = state.update(Message::UpdateRuntimeConnectionFilter("CLIENT-0".into()));
    assert_eq!(state.diag.connections_model.rows().len(), 1);
    assert_eq!(state.diag.connections_model.rows()[0].count, 2);
    assert_eq!(
        state.diag.connections_model.rows()[0].traffic,
        "↑ 2.00 KB / ↓ 4.00 KB"
    );
    let _ = state.update(Message::SetConnectionGroupingMode(
        ConnectionGroupingMode::ByHost,
    ));
    assert_eq!(
        state.diag.connections_model.rows()[0].key,
        "api-0.example.org"
    );
    snapshot
        .connections
        .retain(|row| row.id == "group-duplicate");
    snapshot.connections[0].upload = 4096;
    let _ = state.update(Message::ConnectionsReceived(snapshot));
    assert_eq!(state.diag.connections_model.rows()[0].count, 1);
    assert_eq!(
        state.diag.connections_model.rows()[0].traffic,
        "↑ 4.00 KB / ↓ 2.00 KB"
    );
    let _ = state.update(Message::UpdateRuntimeConnectionFilter("no-match".into()));
    assert!(state.diag.connections_model.rows().is_empty());
    drop(state.view());
}

#[test]
fn grouping_first_observation_respects_existing_search() {
    let (mut state, _) = AppState::demo(&demo_env(Route::Runtime));
    state.runtime.connection_query = "CLIENT-0".into();
    state
        .diag
        .connections_model
        .edit_query(&state.runtime.connection_query);
    let _ = state.update(Message::ConnectionsReceived(grouping_snapshot()));
    let _ = state.update(Message::SetConnectionGroupingMode(
        ConnectionGroupingMode::ByProcess,
    ));
    assert_eq!(state.diag.connections_model.rows().len(), 1);
    assert_eq!(state.diag.connections_model.rows()[0].key, "client-0");
    assert_eq!(state.diag.connections_model.rows()[0].count, 2);
}

#[test]
fn typed_closed_tab_uses_the_surface_snapshot_history_without_string_prefixes() {
    use infiltrator_contract::traffic_history::{
        ClosedConnectionSnapshot, ClosedConnectionsSnapshot,
    };
    use infiltrator_iced::state::runtime::ConnectionTab;

    let (mut state, _) = AppState::demo(&demo_env(Route::Connections));
    let mut snapshot = state
        .surface
        .latest()
        .cloned()
        .expect("demo state publishes a surface snapshot");
    snapshot.revision = snapshot.revision.saturating_add(1);
    snapshot.closed_connections = ClosedConnectionsSnapshot {
        total_closed: 1,
        records: vec![ClosedConnectionSnapshot {
            id: "closed-1".to_owned(),
            destination: "closed.example".to_owned(),
            duration_ms: 1_000,
            upload_bytes: 32,
            download_bytes: 64,
            rule: "DOMAIN-SUFFIX".to_owned(),
            outbound: "PROXY".to_owned(),
            closed_at: "2026-11-12T05:00:00+00:00".to_owned(),
        }],
    };
    assert!(state.apply_shared_surface_snapshot(snapshot));
    assert_eq!(state.diag.closed_connections_model.source_count(), 1);
    assert_eq!(state.runtime.connection_tab, ConnectionTab::Active);

    let _ = state.update(Message::SetConnectionTab(ConnectionTab::Closed));
    assert_eq!(state.runtime.connection_tab, ConnectionTab::Closed);
    let _ = state.update(Message::UpdateRuntimeConnectionFilter("closed".into()));
    assert_eq!(state.diag.closed_connections_model.matched_count(), 1);
    assert_eq!(state.runtime.connection_query, "closed");
    assert!(!state.runtime.connection_query.contains("tab:closed"));
    drop(state.view());
}
