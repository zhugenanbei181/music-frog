//! test-intent: behavior
use crate::state::AppState;
use crate::test_mounts::native_widgets::native;
use crate::types::message::Message;
use crate::view::runtime::connections_controls;
use crate::view_root::interaction_regions::InteractionRegion;
use infiltrator_application::connection_grouping_fixtures::grouping_snapshot;

fn edit(state: &mut AppState, query: &str) {
    let messages = native(
        connections_controls::search(state),
        InteractionRegion::ConnectionSearch.id(),
        Some(query),
    );
    assert!(!messages.is_empty());
    for message in messages {
        let _ = state.update(message);
    }
}
#[test]
fn native_connection_search_preserves_original_runs_metadata_empty_and_clear_through_tea() {
    let (mut state, _) = AppState::new();
    state.shell.demo = true;
    let _ = state.update(Message::ConnectionsReceived(grouping_snapshot()));
    edit(&mut state, "API-0");
    assert_eq!(
        (
            state.diag.connection_groups.matched_count(),
            state.diag.connection_groups.source_count()
        ),
        (2, 9)
    );
    let row = state.diag.connection_groups.search_row("group-0").unwrap();
    assert_eq!(
        row.endpoint
            .iter()
            .filter(|run| run.highlighted)
            .map(|run| run.text.as_str())
            .collect::<Vec<_>>(),
        ["api-0"]
    );
    edit(&mut state, "203.0.113.1");
    let row = state.diag.connection_groups.search_row("group-0").unwrap();
    assert_eq!(row.matched_term[0].text, "203.0.113.1");
    assert!(row.matched_term[0].highlighted);
    edit(&mut state, "no-match");
    assert!(state.diag.connection_groups.summary_empty());
    assert_eq!(state.diag.connection_groups.matched_count(), 0);
    let messages = native(
        connections_controls::search(&state),
        InteractionRegion::ConnectionSearchClear.id(),
        None,
    );
    assert!(!messages.is_empty());
    for message in messages {
        let _ = state.update(message);
    }
    assert_eq!(state.runtime.runtime_connection_filter, "");
    assert_eq!(state.diag.connection_groups.matched_count(), 9);
}

#[test]
fn native_connection_search_refresh_and_core_stop_retire_observations_without_losing_the_query() {
    let (mut state, _) = AppState::new();
    state.shell.demo = true;
    let mut snapshot = grouping_snapshot();
    let _ = state.update(Message::ConnectionsReceived(snapshot.clone()));
    edit(&mut state, "API-0");
    snapshot
        .connections
        .retain(|row| row.id == "group-duplicate");
    let _ = state.update(Message::ConnectionsReceived(snapshot));
    assert_eq!(
        (
            state.diag.connection_groups.matched_count(),
            state.diag.connection_groups.source_count()
        ),
        (1, 1)
    );
    let _ = state.update(Message::ProxyStopped);
    assert_eq!(state.runtime.runtime_connection_filter, "API-0");
    assert!(!state.diag.connection_groups.source_current());
    assert!(!state.diag.connection_groups.summary_empty());
    assert_eq!(state.diag.connection_groups.source_count(), 0);
}
