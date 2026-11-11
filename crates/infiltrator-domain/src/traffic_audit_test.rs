use super::*;

#[test]
fn test_record_flow_and_rankings() {
    let mut auditor = TrafficAuditAccumulator::new();
    let base_time = 1_700_000_000;

    let ev1 = FlowEvent::new(AuditRouteType::Proxied, 1000, 4000)
        .with_process("chrome")
        .with_node("US-Node-1")
        .with_group("AutoSelect")
        .with_domain("youtube.com")
        .with_country("US")
        .with_packets(20)
        .with_timestamp(base_time);
    auditor.record_flow(&ev1);

    let ev2 = FlowEvent::new(AuditRouteType::Proxied, 500, 500)
        .with_process("curl")
        .with_node("HK-Node-1")
        .with_group("ManualProxy")
        .with_domain("github.com")
        .with_country("HK")
        .with_packets(10)
        .with_timestamp(base_time + 10);
    auditor.record_flow(&ev2);

    let ev3 = FlowEvent::new(AuditRouteType::DirectBypass, 2000, 8000)
        .with_process("git")
        .with_domain("gitlab.internal")
        .with_country("CN")
        .with_packets(40)
        .with_timestamp(base_time + 20);
    auditor.record_flow(&ev3);

    // Check Top Processes
    let top_procs = auditor.top_processes_by_traffic(5);
    assert_eq!(top_procs.len(), 3);
    assert_eq!(top_procs[0], ("git".to_string(), 10000));
    assert_eq!(top_procs[1], ("chrome".to_string(), 5000));
    assert_eq!(top_procs[2], ("curl".to_string(), 1000));

    // Check Top Nodes
    let top_nodes = auditor.top_nodes_by_traffic(5);
    assert_eq!(top_nodes.len(), 2);
    assert_eq!(top_nodes[0], ("US-Node-1".to_string(), 5000));
    assert_eq!(top_nodes[1], ("HK-Node-1".to_string(), 1000));

    // Check Top Domains
    let top_doms = auditor.top_domains_by_traffic(5);
    assert_eq!(top_doms.len(), 3);
    assert_eq!(top_doms[0], ("gitlab.internal".to_string(), 10000));
    assert_eq!(top_doms[1], ("youtube.com".to_string(), 5000));
    assert_eq!(top_doms[2], ("github.com".to_string(), 1000));

    // Check Top Countries
    let top_countries = auditor.top_countries_by_traffic(5);
    assert_eq!(top_countries.len(), 3);
    assert_eq!(top_countries[0], ("CN".to_string(), 10000));
    assert_eq!(top_countries[1], ("US".to_string(), 5000));
    assert_eq!(top_countries[2], ("HK".to_string(), 1000));
}

#[test]
fn test_bypass_vs_proxied_ratio() {
    let mut auditor = TrafficAuditAccumulator::new();
    assert_eq!(auditor.bypass_vs_proxied_ratio(), (0.0, 0.0));
    assert_eq!(auditor.direct_bypass_ratio(), 0.0);

    let ev1 = FlowEvent::new(AuditRouteType::Proxied, 2000, 4000)
        .with_process("app1")
        .with_packets(10)
        .with_timestamp(100);
    auditor.record_flow(&ev1);

    let ev2 = FlowEvent::new(AuditRouteType::DirectBypass, 1000, 3000)
        .with_process("app2")
        .with_packets(10)
        .with_timestamp(100);
    auditor.record_flow(&ev2);

    let ev3 = FlowEvent::new(AuditRouteType::Reject, 500, 500)
        .with_process("app3")
        .with_reject_reason(AuditRejectReason::DnsBlock)
        .with_packets(5)
        .with_timestamp(100);
    auditor.record_flow(&ev3);

    let (bypass_ratio, proxied_ratio) = auditor.bypass_vs_proxied_ratio();
    assert!((bypass_ratio - 0.4).abs() < 1e-6);
    assert!((proxied_ratio - 0.6).abs() < 1e-6);
    assert_eq!(auditor.total_proxied_bytes(), 6000);
    assert_eq!(auditor.total_direct_bytes(), 4000);
    assert_eq!(auditor.total_reject_bytes(), 1000);
    assert_eq!(auditor.total_traffic_bytes(), 11000);
}

#[test]
fn test_ewma_rate_estimator() {
    let mut estimator = EwmaRateEstimator::new(0.5);
    assert_eq!(estimator.update(100.0, 0), 0.0);

    // 10,000 bytes transferred in 1 second -> 10,000 Bps
    let r1 = estimator.update(101.0, 10_000);
    assert_eq!(r1, 10_000.0);
    assert_eq!(estimator.instant_rate_bps, 10_000.0);
    assert_eq!(estimator.peak_rate_bps, 10_000.0);

    // 20,000 more bytes in 1 second -> instant 20,000 Bps, EWMA = 0.5 * 20000 + 0.5 * 10000 = 15000
    let r2 = estimator.update(102.0, 30_000);
    assert_eq!(r2, 15_000.0);
    assert_eq!(estimator.peak_rate_bps, 20_000.0);
    assert!((estimator.mbps() - 0.12).abs() < 1e-4);
}

#[test]
fn test_latency_distribution_and_jitter() {
    let mut tracker = LatencyDistributionTracker::new();
    assert!(tracker.compute_summary().is_none());

    tracker.record_sample(50);
    tracker.record_sample(60);
    tracker.record_sample(55);
    tracker.record_sample(70);
    tracker.record_sample(100);

    let summary = tracker.compute_summary().unwrap();
    assert_eq!(summary.sample_count, 5);
    assert_eq!(summary.min_ms, 50);
    assert_eq!(summary.max_ms, 100);
    assert_eq!(summary.p50_ms, 60.0);
    assert!(summary.p90_ms > 70.0);
    assert!(summary.avg_ms > 60.0);
    assert!(summary.jitter_ms > 0.0);
}

#[test]
fn test_timeseries_bucketing_and_pruning() {
    let mut auditor = TrafficAuditAccumulator::new();
    let hour_1 = 3600 * 10;
    let hour_2 = 3600 * 11;
    let hour_3 = 3600 * 12;

    let ev1 = FlowEvent::new(AuditRouteType::Proxied, 100, 200)
        .with_process("chrome")
        .with_node("Node-1")
        .with_domain("example.com")
        .with_packets(2)
        .with_timestamp(hour_1 + 50);
    auditor.record_flow(&ev1);

    let ev2 = FlowEvent::new(AuditRouteType::Proxied, 300, 400)
        .with_process("chrome")
        .with_node("Node-1")
        .with_domain("example.com")
        .with_packets(4)
        .with_timestamp(hour_2 + 100);
    auditor.record_flow(&ev2);

    let ev3 = FlowEvent::new(AuditRouteType::Proxied, 500, 600)
        .with_process("chrome")
        .with_node("Node-1")
        .with_domain("example.com")
        .with_packets(6)
        .with_timestamp(hour_3 + 200);
    auditor.record_flow(&ev3);

    let series = auditor.get_process_hourly_series("chrome", hour_1, hour_3);
    assert_eq!(series.len(), 3);
    assert_eq!(series[0].0, hour_1);
    assert_eq!(series[0].1.total_bytes(), 300);
    assert_eq!(series[1].0, hour_2);
    assert_eq!(series[1].1.total_bytes(), 700);
    assert_eq!(series[2].0, hour_3);
    assert_eq!(series[2].1.total_bytes(), 1100);

    let dom_series = auditor.get_domain_hourly_series("example.com", hour_1, hour_3);
    assert_eq!(dom_series.len(), 3);

    // Prune older than hour_2
    let pruned = auditor.prune_older_than(hour_2);
    assert!(pruned > 0);

    let series_after = auditor.get_process_hourly_series("chrome", hour_1, hour_3);
    assert_eq!(series_after.len(), 2);
    assert_eq!(series_after[0].0, hour_2);
}

#[test]
fn test_prometheus_and_snapshot() {
    let mut auditor = TrafficAuditAccumulator::new();
    auditor.record_flow(
        &FlowEvent::new(AuditRouteType::Proxied, 1000, 2000)
            .with_process("curl")
            .with_node("HK-1")
            .with_domain("api.github.com")
            .with_country("HK"),
    );

    let prom = auditor.export_prometheus_metrics();
    assert!(
        prom.contains(
            "infiltrator_traffic_bytes_total{route=\"proxied\",direction=\"upload\"} 1000"
        )
    );
    assert!(prom.contains(
        "infiltrator_traffic_bytes_total{route=\"proxied\",direction=\"download\"} 2000"
    ));

    let snap = auditor.snapshot();
    assert_eq!(snap.total_proxied_bytes, 3000);
    assert_eq!(snap.top_processes[0].0, "curl");
    assert_eq!(snap.top_nodes[0].0, "HK-1");
    assert_eq!(snap.top_domains[0].0, "api.github.com");
    assert_eq!(snap.top_countries[0].0, "HK");
}

#[test]
fn test_clear_and_empty() {
    let mut auditor = TrafficAuditAccumulator::new();
    auditor.record_process_flow("node", 100, 100, 2, AuditRouteType::Proxied);
    assert_eq!(auditor.top_processes_by_traffic(1).len(), 1);

    auditor.clear();
    assert!(auditor.top_processes_by_traffic(1).is_empty());
    assert_eq!(auditor.total_traffic_bytes(), 0);
}

#[test]
fn one_index_serves_every_dimension() {
    let mut auditor = TrafficAuditAccumulator::new();
    let base_time = 1_700_000_000;
    auditor.record_flow(
        &FlowEvent::new(AuditRouteType::Proxied, 1000, 4000)
            .with_process("chrome")
            .with_node("HK-01")
            .with_group("AutoSelect")
            .with_domain("github.com")
            .with_country("HK")
            .with_device("127.0.0.1")
            .with_packets(20)
            .with_timestamp(base_time),
    );

    // The named accessors delegate to the generic read, so the two must agree
    // for every dimension the accumulator records.
    assert_eq!(
        auditor.top(TrafficDimension::Process, 5),
        auditor.top_processes_by_traffic(5)
    );
    assert_eq!(
        auditor.top(TrafficDimension::Node, 5),
        auditor.top_nodes_by_traffic(5)
    );
    assert_eq!(
        auditor.top(TrafficDimension::Group, 5),
        auditor.top_groups_by_traffic(5)
    );
    assert_eq!(
        auditor.top(TrafficDimension::Domain, 5),
        auditor.top_domains_by_traffic(5)
    );
    assert_eq!(
        auditor.top(TrafficDimension::Country, 5),
        auditor.top_countries_by_traffic(5)
    );
    assert_eq!(
        auditor.top(TrafficDimension::Device, 5),
        vec![("127.0.0.1".to_string(), 5000)]
    );

    assert_eq!(
        auditor
            .stats_of(TrafficDimension::Device, "127.0.0.1")
            .map(|stats| stats.total_bytes()),
        Some(5000)
    );
    assert_eq!(
        auditor.get_device_traffic("127.0.0.1"),
        auditor.stats_of(TrafficDimension::Device, "127.0.0.1")
    );
    assert_eq!(
        auditor.get_process_traffic("chrome"),
        auditor.stats_of(TrafficDimension::Process, "chrome")
    );

    // The dimensions stay separate: a device name is not a process name.
    assert_eq!(auditor.top_processes_by_traffic(5).len(), 1);
    assert_eq!(auditor.top_devices_by_traffic(5).len(), 1);
}

#[test]
fn a_flow_carries_exactly_the_dimensions_it_reported() {
    let event = FlowEvent::new(AuditRouteType::Proxied, 1, 1)
        .with_process("chrome")
        .with_country("HK")
        .with_device("127.0.0.1");
    assert_eq!(
        event.attributions(),
        vec![
            (TrafficDimension::Process, "chrome".to_string()),
            (TrafficDimension::Country, "HK".to_string()),
            (TrafficDimension::Device, "127.0.0.1".to_string()),
        ]
    );
}

// A source address the collector did not report is not an entity. The
// accumulator used to have one hand-written block per dimension, where "no
// address" could only be expressed by a branch; the dimension list makes the
// absence explicit, and this is the test that keeps it that way.
#[test]
fn a_device_that_was_not_reported_attributes_nothing() {
    let mut auditor = TrafficAuditAccumulator::new();
    let event = FlowEvent::new(AuditRouteType::Proxied, 10, 20)
        .with_process("chrome")
        .with_device("   ");
    assert!(
        !event
            .attributions()
            .iter()
            .any(|(dimension, _)| *dimension == TrafficDimension::Device)
    );

    auditor.record_flow(&event);
    assert!(auditor.top_devices_by_traffic(5).is_empty());
    assert!(auditor.get_device_traffic("   ").is_none());
    assert!(auditor.get_device_traffic("").is_none());
    assert_eq!(auditor.top_processes_by_traffic(5).len(), 1);
}

#[test]
fn the_series_window_is_one_read_for_every_dimension() {
    let mut auditor = TrafficAuditAccumulator::new();
    let hour_1 = 3600 * 10;
    let hour_2 = 3600 * 11;
    let day_1 = 86_400;

    auditor.record_flow(
        &FlowEvent::new(AuditRouteType::Proxied, 100, 200)
            .with_process("chrome")
            .with_node("Node-1")
            .with_device("10.0.0.2")
            .with_packets(2)
            .with_timestamp(hour_1 + 50),
    );
    auditor.record_flow(
        &FlowEvent::new(AuditRouteType::Proxied, 300, 400)
            .with_process("chrome")
            .with_node("Node-1")
            .with_device("10.0.0.2")
            .with_packets(4)
            .with_timestamp(hour_2 + 100),
    );

    assert_eq!(
        auditor.hourly_series(TrafficDimension::Process, "chrome", hour_1, hour_2),
        auditor.get_process_hourly_series("chrome", hour_1, hour_2)
    );
    assert_eq!(
        auditor.hourly_series(TrafficDimension::Device, "10.0.0.2", hour_1, hour_2),
        auditor.get_process_hourly_series("chrome", hour_1, hour_2)
    );
    assert_eq!(
        auditor.daily_series(TrafficDimension::Node, "Node-1", day_1, day_1 + 86_400),
        auditor.get_node_daily_series("Node-1", day_1, day_1 + 86_400)
    );

    // A window that excludes the buckets reads nothing, for every dimension.
    assert!(
        auditor
            .hourly_series(
                TrafficDimension::Device,
                "10.0.0.2",
                hour_2 + 3600,
                hour_2 + 7200
            )
            .is_empty()
    );
}
