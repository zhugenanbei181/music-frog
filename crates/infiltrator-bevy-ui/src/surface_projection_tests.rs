//! The projection's own regressions: what the surface reads out of the
//! owner's snapshot.

use super::*;
use lurker_core_contract::outbound::preferences::OutboundSortOrder;

use super::super::surface_demo::demo_snapshot;

/// The engine identity of one demo outbound node.
fn demo_outbound_id(snapshot: &surface_snapshot::SurfaceSnapshot, value: &str) -> OutboundId {
    let inventory = snapshot
        .outbound_inventory
        .data
        .as_ref()
        .expect("the demo snapshot publishes the outbound inventory");
    let OutboundInventoryState::Ready { entries, .. } = inventory.inventory() else {
        panic!("the demo outbound inventory is ready");
    };
    entries
        .iter()
        .find(|entry| entry.id.value() == value)
        .unwrap_or_else(|| panic!("the demo snapshot publishes {value}"))
        .id
        .clone()
}

#[test]
fn the_proxies_projection_keeps_the_favourite_first_on_every_order() {
    let mut snapshot = demo_snapshot();
    let slowest = demo_outbound_id(&snapshot, "US-04");
    let preferences = snapshot
        .outbound_ui_preferences
        .data
        .as_mut()
        .expect("the demo snapshot publishes the outbound preferences");
    preferences.favorite_outbound_ids = vec![slowest];
    preferences.sort_order = OutboundSortOrder::NameAscending;

    let projection = proxies_projection(&snapshot);
    let group = projection
        .groups
        .iter()
        .find(|group| group.name == "PROXIES")
        .expect("the demo snapshot publishes the PROXIES group");
    let names: Vec<&str> = group
        .proxies
        .iter()
        .map(|node| node.name.as_str())
        .collect();

    // The shared member order puts a favourite first on every order —
    // the name order included, which the former local rule did not.
    assert_eq!(
        names,
        vec![
            "US-Silicon-Valley 04 · IEPL 专线 · 4K 加速",
            "HK-01",
            "JP-02",
            "SG-03"
        ]
    );
}

#[test]
fn the_proxies_projection_filters_the_members_with_the_shared_rule() {
    let mut snapshot = demo_snapshot();
    let preferences = snapshot
        .outbound_ui_preferences
        .data
        .as_mut()
        .expect("the demo snapshot publishes the outbound preferences");
    preferences.search_query = "trojan".to_owned();

    let projection = proxies_projection(&snapshot);
    let group = projection
        .groups
        .iter()
        .find(|group| group.name == "PROXIES")
        .expect("the demo snapshot publishes the PROXIES group");
    let names: Vec<&str> = group
        .proxies
        .iter()
        .map(|node| node.name.as_str())
        .collect();

    // The shared rule matches the protocol, so the group that does not
    // match the query keeps only the member the query matches.
    assert_eq!(names, vec!["SG-03"]);
}

#[test]
fn the_proxies_projection_highlights_with_the_shared_name_runs() {
    let mut snapshot = demo_snapshot();
    let preferences = snapshot
        .outbound_ui_preferences
        .data
        .as_mut()
        .expect("the demo snapshot publishes the outbound preferences");
    preferences.search_query = "jp".to_owned();

    let projection = proxies_projection(&snapshot);
    let runs = projection
        .name_runs
        .get("JP-02")
        .expect("the matching node carries its highlight runs");
    assert!(
        runs.iter().any(|run| run.highlighted),
        "the shared name runs highlight the matched node"
    );
}

#[test]
fn the_proxies_projection_publishes_the_owner_order_and_the_latency_history() {
    let mut snapshot = demo_snapshot();
    let preferences = snapshot
        .outbound_ui_preferences
        .data
        .as_mut()
        .expect("the demo snapshot publishes the outbound preferences");
    preferences.sort_order = OutboundSortOrder::NameDescending;

    let projection = proxies_projection(&snapshot);
    // The sort strip seeds its selection from the order the owner
    // publishes, so the projection carries that order.
    assert_eq!(projection.sort_order, OutboundSortOrder::NameDescending);

    let group = projection
        .groups
        .iter()
        .find(|group| group.name == "PROXIES")
        .expect("the demo snapshot publishes the PROXIES group");
    let node = group
        .proxies
        .iter()
        .find(|node| node.name == "HK-01")
        .expect("the demo snapshot publishes the HK-01 node");
    // The node carries the owner's recorded latency history, so the page
    // draws the sparkline from real samples instead of inventing one.
    assert_eq!(node.latency_samples.len(), 3);
    assert_eq!(
        node.latency_samples.last().map(|sample| sample.delay_ms),
        Some(38)
    );
}
