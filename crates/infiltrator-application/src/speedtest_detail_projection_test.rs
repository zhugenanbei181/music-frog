use super::*;
use infiltrator_contract::capability::Availability;

#[test]
fn detail_rows_preserve_order_metrics_missing_facts_and_all_five_stars() {
    let snapshot = SpeedtestSnapshot::demo_fixture();
    let details = project_details(&snapshot);
    assert_eq!(details.rows.len(), snapshot.node_count());
    for (row, node) in details.rows.iter().zip(snapshot.sorted_by_latency()) {
        assert_eq!(row.node_name, node.node_name);
        assert_eq!(row.proxy_type, node.proxy_type);
        assert_eq!(row.egress, node.egress_endpoint_label());
        assert_eq!(row.egress_match, node.egress_country_match());
        assert_eq!(row.stars.chars().count(), 5);
    }
    let dead = details.rows.iter().find(|r| r.delay == "—").unwrap();
    assert_eq!(dead.bandwidth, "—");
    assert_eq!(dead.jitter, "—");
    assert_eq!(dead.loss, "100.0%");
    assert_eq!(dead.egress, "—");
    assert_eq!(details.availability, Some(Availability::Supported));
    assert!(listing(&details, &english).contains("Mbps"));
}

#[test]
fn unknown_empty_failure_and_typed_unsupported_remain_distinct() {
    let mut snapshot = SpeedtestSnapshot::default();
    let lang = english;
    assert_eq!(project_details(&snapshot).availability, None);
    assert_eq!(
        listing(&project_details(&snapshot), &lang),
        "No speedtest results yet"
    );
    snapshot.failure = Some("probe refused".into());
    assert_eq!(
        listing(&project_details(&snapshot), &lang),
        "Speedtest failed: probe refused"
    );
    snapshot.availability = Some(Availability::Unsupported {
        reason: "missing host engine".into(),
    });
    assert_eq!(
        listing(&project_details(&snapshot), &lang),
        "Speedtest unsupported by this host: missing host engine"
    );
    snapshot.availability = Some(Availability::Unavailable {
        reason: "host disconnected".into(),
    });
    assert_eq!(
        listing(&project_details(&snapshot), &lang),
        "Speedtest engine unavailable: host disconnected"
    );
}

fn english(key: &str) -> String {
    match key {
        "speedtest_detail_empty" => "No speedtest results yet",
        "speedtest_detail_failed" => "Speedtest failed",
        "speedtest_detail_unsupported" => "Speedtest unsupported by this host",
        "speedtest_detail_unavailable" => "Speedtest engine unavailable",
        _ => key,
    }
    .into()
}

#[test]
fn detail_partial_probes_keep_unknown_stability_and_measured_zero() {
    use infiltrator_contract::speedtest::JitterCalculation;
    let mut snapshot = SpeedtestSnapshot::demo_fixture();
    let mut node = snapshot.fastest_node().unwrap().clone();
    snapshot.node_results.clear();
    node.jitter = None;
    node.bandwidth_mbps = Some(0.0);
    snapshot
        .node_results
        .insert(node.node_name.clone(), node.clone());
    let row = project_details(&snapshot).rows.remove(0);
    assert_eq!(
        (row.jitter.as_str(), row.loss.as_str(), row.stars.as_str()),
        ("—", "—", "—")
    );
    assert_eq!(row.bandwidth, "0.0 Mbps");
    node.jitter = Some(JitterCalculation::from_samples(&[Some(0), Some(0)]));
    snapshot.node_results.insert(node.node_name.clone(), node);
    let row = project_details(&snapshot).rows.remove(0);
    assert_eq!((row.jitter.as_str(), row.loss.as_str()), ("0.0 ms", "0.0%"));
    assert_eq!(row.stars.chars().count(), 5);
}
