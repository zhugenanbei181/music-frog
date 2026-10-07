//! test-intent: behavior
use crate::connection_grouping::ConnectionGroupingState;
use crate::connection_grouping_fixtures::grouping_snapshot;
use crate::connection_rate_application::project_connection;
use crate::connection_search::project_search_row;

#[test]
fn live_and_projected_facts_have_identical_case_insensitive_runs_and_metadata_matches() {
    let snapshot = grouping_snapshot();
    let live = &snapshot.connections[0];
    let projected = project_connection(live, Default::default());
    for query in [
        "API-0",
        "CLIENT-0",
        "203.0.113.1",
        "/usr/bin/client-0",
        "absent",
        "",
    ] {
        assert_eq!(
            project_search_row(live, query),
            project_search_row(&projected, query)
        );
    }
    let host = project_search_row(live, "API-0");
    assert!(host.visible);
    assert_eq!(
        host.endpoint
            .iter()
            .map(|run| run.text.as_str())
            .collect::<String>(),
        "api-0.example.org:443"
    );
    assert_eq!(
        host.endpoint
            .iter()
            .filter(|run| run.highlighted)
            .map(|run| run.text.as_str())
            .collect::<Vec<_>>(),
        ["api-0"]
    );
    let metadata = project_search_row(live, "203.0.113.1");
    assert!(metadata.visible);
    assert_eq!(metadata.matched_term[0].text, "203.0.113.1");
    assert!(metadata.matched_term[0].highlighted);
    let path = project_search_row(live, "/usr/bin/client-0");
    assert_eq!(path.matched_term[0].text, "/usr/bin/client-0");
    assert!(!project_search_row(live, "absent").visible);
}

#[test]
fn cached_search_counts_follow_refresh_and_distinguish_unavailable_from_observed_empty() {
    let mut snapshot = grouping_snapshot();
    let mut owner = ConnectionGroupingState::default();
    owner.edit_query("absent");
    assert!(!owner.summary_empty());
    owner.observe(&snapshot.connections);
    assert_eq!((owner.matched_count(), owner.source_count()), (0, 9));
    assert!(owner.summary_empty());
    owner.edit_query("api-0");
    assert_eq!(owner.matched_count(), 2);
    owner.mark_unavailable();
    assert_eq!(owner.source_count(), 9);
    assert_eq!(owner.matched_count(), 2);
    assert!(!owner.summary_empty());
    assert_eq!(owner.search_summary_key(), "conn_search_unavailable");
    snapshot
        .connections
        .retain(|row| row.id == "group-duplicate");
    owner.observe(&snapshot.connections);
    assert_eq!((owner.matched_count(), owner.source_count()), (1, 1));
    owner.observe(&[]);
    assert!(owner.summary_empty());
    assert_eq!((owner.matched_count(), owner.source_count()), (0, 0));
    owner.clear_observation();
    assert!(!owner.summary_empty());
    assert!(!owner.source_current());
    assert_eq!(owner.query(), "api-0");
}
