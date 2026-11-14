//! test-intent: behavior
//! A group's own filter narrows only that group, and the highlighted name
//! runs follow the query of the group that shows the node.

use crate::state::AppState;
use crate::test_mounts::command_harness::{install_test_core_capabilities, recording_application};
use crate::test_mounts::outbound_fixtures::{
    OutboundFixture, OutboundFixtureInput, standard_members,
};
use crate::types::message::Message;
use crate::types::message::proxy_page_intent::ProxyPageIntent;
use infiltrator_contract::core_capabilities::{CoreOperation, OutboundOperation};
use lurker_core_contract::outbound::preferences::{
    OutboundHealthFilter, OutboundLayout, OutboundSortOrder,
};

fn product_state() -> AppState {
    let (application, _) = recording_application();
    install_test_core_capabilities(
        &application,
        [CoreOperation::Outbound(OutboundOperation::List)],
    );
    let scope = application
        .snapshot()
        .scope()
        .expect("identified owner scope");
    let members = standard_members();
    let fixture = OutboundFixture::new(OutboundFixtureInput {
        scope: &scope,
        revision: 41,
        members: &members,
        selected: Some("proxy-a"),
        favorites: &[],
        health_filter: OutboundHealthFilter::All,
        sort_order: OutboundSortOrder::NameAscending,
        layout: OutboundLayout::Grid,
    });
    let mut state = AppState::empty();
    fixture.attach_to(&mut state, &application);
    state
}

fn visible(state: &AppState, group: &str) -> Vec<String> {
    state
        .runtime
        .filtered_groups
        .iter()
        .find(|(name, _)| name == group)
        .map(|(_, members)| members.clone())
        .expect("the group is on the page")
}

#[test]
fn a_group_filter_narrows_only_its_own_members_and_highlights_them() {
    let mut state = product_state();
    let baseline = visible(&state, "GLOBAL");
    assert!(baseline.len() >= 4, "the fixture publishes a node list");

    let _ = state.update(Message::ProxyPageIntent(
        ProxyPageIntent::ToggleGroupFilter("GLOBAL".into()),
    ));
    assert!(state.runtime.proxy_group_filters.is_open("GLOBAL"));
    assert_eq!(
        visible(&state, "GLOBAL"),
        baseline,
        "an empty group filter keeps every node"
    );

    let _ = state.update(Message::ProxyPageIntent(ProxyPageIntent::FilterGroup {
        group: "GLOBAL".into(),
        text: "HK".into(),
    }));
    let narrowed = visible(&state, "GLOBAL");
    assert!(
        narrowed.len() < baseline.len(),
        "the group's own query narrows its members"
    );
    assert!(narrowed.iter().any(|member| member == "hong-kong"));
    assert!(
        state
            .runtime
            .proxy_name_runs
            .get(&("GLOBAL".to_owned(), "hong-kong".to_owned()))
            .is_some_and(|runs| runs.iter().any(|run| run.highlighted)),
        "the group highlights by its own query"
    );
}

#[test]
fn closing_the_group_filter_restores_the_members_and_the_page_query() {
    let mut state = product_state();
    let baseline = visible(&state, "GLOBAL");
    let _ = state.update(Message::ProxyPageIntent(
        ProxyPageIntent::ToggleGroupFilter("GLOBAL".into()),
    ));
    let _ = state.update(Message::ProxyPageIntent(ProxyPageIntent::FilterGroup {
        group: "GLOBAL".into(),
        text: "HK".into(),
    }));
    assert!(visible(&state, "GLOBAL").len() < baseline.len());

    let _ = state.update(Message::ProxyPageIntent(
        ProxyPageIntent::ToggleGroupFilter("GLOBAL".into()),
    ));
    assert_eq!(visible(&state, "GLOBAL"), baseline);
    assert_eq!(state.runtime.proxy_group_filters.query("GLOBAL"), None);
}

#[test]
fn a_group_with_a_filter_that_hides_every_member_stays_on_the_page() {
    let mut state = product_state();
    let _ = state.update(Message::ProxyPageIntent(
        ProxyPageIntent::ToggleGroupFilter("GLOBAL".into()),
    ));
    let _ = state.update(Message::ProxyPageIntent(ProxyPageIntent::FilterGroup {
        group: "GLOBAL".into(),
        text: "no-such-node".into(),
    }));
    assert_eq!(visible(&state, "GLOBAL"), Vec::<String>::new());
    assert!(
        state
            .runtime
            .filtered_groups
            .iter()
            .any(|(name, _)| name == "GLOBAL"),
        "an empty result still shows the box so the user can clear it"
    );
}
