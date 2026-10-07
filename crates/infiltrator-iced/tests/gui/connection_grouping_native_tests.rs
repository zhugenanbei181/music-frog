//! test-intent: behavior
use crate::state::AppState;
use crate::test_mounts::native_widgets::native;
use crate::types::message::Message;
use crate::view::runtime::connections_controls;
use crate::view_root::interaction_regions::InteractionRegion;
use infiltrator_application::connection_grouping_fixtures::grouping_snapshot;
use infiltrator_domain::connection_view::ConnectionGroupingMode;

fn feed(state: &mut AppState, messages: Vec<Message>) {
    assert!(
        !messages.is_empty(),
        "native input must publish actual messages"
    );
    for message in messages {
        let _ = state.update(message);
    }
}
fn choose(state: &mut AppState, region: InteractionRegion) {
    let messages = native(connections_controls::grouping(state), region.id(), None);
    feed(state, messages);
}

#[test]
fn native_grouping_controls_preserve_filtered_counts_across_dimensions_and_clear() {
    let (mut state, _) = AppState::new();
    state.shell.demo = true;
    let _ = state.update(Message::ConnectionsReceived(grouping_snapshot()));
    choose(&mut state, InteractionRegion::ConnectionGroupProcess);
    assert_eq!(state.diag.connection_groups.rows().len(), 8);
    let messages = native(
        connections_controls::search(&state),
        InteractionRegion::ConnectionSearch.id(),
        Some("CLIENT-0"),
    );
    feed(&mut state, messages);
    assert_eq!(state.runtime.runtime_connection_filter, "CLIENT-0");
    assert_eq!(state.diag.connection_groups.rows().len(), 1);
    assert_eq!(state.diag.connection_groups.rows()[0].count, 2);
    choose(&mut state, InteractionRegion::ConnectionGroupHost);
    assert_eq!(
        state.diag.connection_groups.rows()[0].key,
        "api-0.example.org"
    );
    choose(&mut state, InteractionRegion::ConnectionGroupFlat);
    assert_eq!(
        state.diag.connection_groups.mode(),
        ConnectionGroupingMode::Flat
    );
    assert_eq!(state.runtime.runtime_connection_filter, "CLIENT-0");
    let messages = native(
        connections_controls::search(&state),
        InteractionRegion::ConnectionSearchClear.id(),
        None,
    );
    feed(&mut state, messages);
    choose(&mut state, InteractionRegion::ConnectionGroupProcess);
    assert_eq!(state.runtime.runtime_connection_filter, "");
    assert_eq!(state.diag.connection_groups.rows().len(), 8);
}
