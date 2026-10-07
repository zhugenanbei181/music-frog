use super::{dead_archive, egress_label, history_lines, metrics_label, node_metrics, rating_label};
use infiltrator_contract::speedtest::{JitterCalculation, SpeedtestScope, SpeedtestSnapshot};

#[test]
fn partial_probes_cannot_claim_packet_loss_or_stability_and_measured_zero_is_retained() {
    let snapshot = SpeedtestSnapshot::demo_fixture();
    let mut node = snapshot.fastest_node().unwrap().clone();
    node.node_name = "node {rating}/中文🙂".into();
    node.bandwidth_mbps = Some(0.0);
    node.jitter = None;
    let metrics = node_metrics(&node);
    assert_eq!(metrics.bandwidth, "0.0 Mbps");
    assert_eq!(metrics.jitter, "—");
    assert_eq!(metrics.loss, "—");
    assert!(metrics.rating.is_none());
    assert_eq!(metrics.stars, "—");
    for locale in ["zh-CN", "en-US"] {
        let label = metrics_label(Some(&node), locale);
        assert!(label.starts_with("node {rating}/中文🙂"));
        assert!(!label.contains("Excellent"));
        assert!(!label.contains("极佳"));
    }
    for samples in [&[None, None][..], &[Some(0), None][..]] {
        node.jitter = Some(JitterCalculation::from_samples(samples));
        let metrics = node_metrics(&node);
        assert_eq!(
            metrics.jitter, "—",
            "jitter needs a pair of successful RTTs"
        );
        assert_eq!(
            metrics.loss,
            if samples[0].is_some() {
                "50.0%"
            } else {
                "100.0%"
            }
        );
        assert!(metrics.rating.is_some());
    }
    node.jitter = Some(JitterCalculation::from_samples(&[Some(0), Some(0)]));
    let metrics = node_metrics(&node);
    assert_eq!(metrics.jitter, "0.0 ms");
    assert_eq!(metrics.loss, "0.0%");
    assert_eq!(rating_label(metrics.rating, "en-US"), "Excellent (0%)");
    assert_eq!(metrics.stars.chars().count(), 5);
}

#[test]
fn history_keeps_reverse_order_opaque_scope_unknown_values_and_out_of_range_time() {
    let mut snapshot = SpeedtestSnapshot::demo_fixture();
    let mut first = snapshot.recent_history[0].clone();
    first.scope = SpeedtestScope::SingleGroup("first {time}/中文🙂".into());
    first.avg_latency_ms = Some(0.0);
    first.avg_jitter_ms = None;
    first.avg_bandwidth_mbps = None;
    let mut second = first.clone();
    second.scope = SpeedtestScope::SingleNode("second {scope}/🙂".into());
    second.timestamp_epoch_ms = u64::MAX;
    snapshot.recent_history = vec![first, second];
    let lines = history_lines(&snapshot, "en-US");
    assert_eq!(lines.len(), 2);
    assert!(lines[0].starts_with("— · Node second {scope}/🙂"));
    assert!(lines[1].contains("Group first {time}/中文🙂"));
    assert!(lines[1].contains("Avg latency 0.0 ms"));
    assert!(lines[1].contains("Avg jitter —"));
    assert!(lines[1].contains("Avg bandwidth —"));
}

#[test]
fn archive_reports_full_count_and_explicit_bounded_names_without_translating_egress() {
    let mut snapshot = SpeedtestSnapshot::demo_fixture();
    let base = snapshot.dead_nodes()[0].clone();
    snapshot.node_results.clear();
    for index in 0..6 {
        let mut node = base.clone();
        node.node_name = format!("dead {index} {{count}}/🙂");
        snapshot.node_results.insert(index.to_string(), node);
    }
    let archive = dead_archive(&snapshot, "en-US");
    assert_eq!(archive.count, 6);
    assert!(archive.names.contains("dead 0 {count}/🙂"));
    assert!(archive.names.ends_with("(+2)"));
    assert!(!archive.names.contains("dead 5"));
    assert!(archive.caption.starts_with("Unreachable 6"));
    let mut node = base;
    node.outbound_ip = Some("endpoint {status}/🙂".into());
    node.outbound_country = None;
    assert_eq!(
        egress_label(Some(&node), "en-US"),
        "Egress endpoint {status}/🙂 · Egress not probed"
    );
}
