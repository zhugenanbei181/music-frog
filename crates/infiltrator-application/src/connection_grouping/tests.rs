//! test-intent: behavior
use crate::connection_grouping::ConnectionGroupingState;
use crate::connection_grouping_fixtures::{grouping_snapshot, grouping_surface};
use crate::connection_rate_application::project_connection;
use infiltrator_contract::error::Failure;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_domain::connection_view::{self, ConnectionGroupingMode, ConnectionSortKey};

#[test]
fn grouping_preserves_all_buckets_and_identical_projection_semantics() {
    let snapshot = grouping_snapshot();
    let projected: Vec<_> = snapshot
        .connections
        .iter()
        .map(|row| project_connection(row, Default::default()))
        .collect();
    let mut live = ConnectionGroupingState::default();
    let mut contract = ConnectionGroupingState::default();
    live.observe(&snapshot.connections);
    contract.observe(&projected);
    for mode in [
        ConnectionGroupingMode::ByProcess,
        ConnectionGroupingMode::ByHost,
    ] {
        live.select(mode);
        contract.select(mode);
        assert_eq!(live.rows(), contract.rows());
        assert_eq!(live.rows().len(), 8);
        assert_eq!(live.rows().iter().map(|row| row.count).sum::<usize>(), 9);
        assert_eq!(live.rows()[0].traffic, "↑ 8.00 KB / ↓ 16.00 KB");
        let duplicate = live.rows().iter().find(|row| row.count == 2).unwrap();
        assert_eq!(duplicate.traffic, "↑ 2.00 KB / ↓ 4.00 KB");
    }
}

#[test]
fn grouping_search_survives_dimension_switch_and_refresh_without_stale_rows() {
    let mut snapshot = grouping_snapshot();
    let mut state = ConnectionGroupingState::default();
    state.edit_query("CLIENT-0");
    state.observe(&snapshot.connections);
    state.select(ConnectionGroupingMode::ByProcess);
    assert_eq!(state.rows().len(), 1);
    assert_eq!(
        (state.rows()[0].key.as_str(), state.rows()[0].count),
        ("client-0", 2)
    );
    state.select(ConnectionGroupingMode::ByHost);
    assert_eq!(state.rows()[0].key, "api-0.example.org");
    snapshot
        .connections
        .retain(|row| row.id == "group-duplicate");
    snapshot.connections[0].upload = 4096;
    state.observe(&snapshot.connections);
    assert_eq!(state.rows()[0].count, 1);
    assert_eq!(state.rows()[0].traffic, "↑ 4.00 KB / ↓ 2.00 KB");
    state.edit_query("no-match");
    assert!(state.rows().is_empty());
    assert_eq!(state.summary_key(), "conn_aggregate_empty");
    state.edit_query("");
    state.select(ConnectionGroupingMode::Flat);
    assert!(state.rows().is_empty());
    state.select(ConnectionGroupingMode::ByProcess);
    assert_eq!(state.rows()[0].count, 1);
}

#[test]
fn connection_projection_preserves_ip_search_and_latest_sort() {
    let snapshot = grouping_snapshot();
    let mut projected: Vec<_> = snapshot
        .connections
        .iter()
        .map(|row| project_connection(row, Default::default()))
        .collect();
    let mut live = snapshot.connections.clone();
    for query in [
        "203.0.113.1",
        "192.0.2.5",
        "51000",
        "tcp",
        "/usr/bin/client-0",
    ] {
        let expected: Vec<_> = live
            .iter()
            .filter(|row| connection_view::matches_search(*row, query))
            .map(|row| row.id.as_str())
            .collect();
        let actual: Vec<_> = projected
            .iter()
            .filter(|row| connection_view::matches_search(*row, query))
            .map(|row| row.id.as_str())
            .collect();
        assert_eq!(actual, expected, "search facts for {query}");
        assert!(
            !actual.is_empty(),
            "the fixture contains the queried observation"
        );
    }
    connection_view::sort_connections(&mut live, ConnectionSortKey::LatestDesc);
    connection_view::sort_connections(&mut projected, ConnectionSortKey::LatestDesc);
    assert_eq!(live[0].id, "group-7");
    assert_eq!(projected[0].start, "2026-10-06T00:00:07Z");
    assert_eq!(
        live.iter().map(|row| &row.id).collect::<Vec<_>>(),
        projected.iter().map(|row| &row.id).collect::<Vec<_>>()
    );
}

#[test]
fn grouping_capture_publishes_one_count_for_the_page_core_and_shell() {
    let base = SurfaceSnapshot::unavailable(
        SurfaceKind::BevyDesktop,
        HostKind::Desktop,
        Failure::unsupported("isolated fixture"),
    );
    let surface = grouping_surface(base);
    let page = surface.pages.connections.data.as_ref().unwrap();
    assert_eq!(page.total_connections, 9);
    assert_eq!(page.connections.len(), 9);
    assert_eq!(surface.core.active_connections, 9);
    assert_eq!(surface.shell_readout.connections.value, Some(9));
    assert!(surface.shell_readout.connections.current);
    assert_eq!(
        (page.total_upload_bytes, page.total_download_bytes),
        (37 * 1024, 74 * 1024)
    );
    let mut groups = ConnectionGroupingState::default();
    groups.observe(&page.connections);
    groups.edit_query("CLIENT-0");
    groups.select(ConnectionGroupingMode::ByProcess);
    assert_eq!(groups.rows().len(), 1);
    assert_eq!(groups.rows()[0].count, 2);
}
