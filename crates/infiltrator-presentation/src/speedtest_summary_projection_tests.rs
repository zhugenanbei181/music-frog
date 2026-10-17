use super::{dead_archive, history_lines, metrics_label, node_metrics};
use lurker_core_contract::identity::EngineId;
use lurker_core_contract::outbound::identity::OutboundId;
use infiltrator_contract::speedtest::{
    HistoricalSpeedtestRecord, SpeedtestConcurrency, SpeedtestOutboundResult,
    SpeedtestResultState, SpeedtestRunId, SpeedtestSampleOutcome, SpeedtestSnapshot,
};

fn outbound_result(
    id: &str,
    display_name: &str,
    samples: Vec<SpeedtestSampleOutcome>,
    state: SpeedtestResultState,
) -> SpeedtestOutboundResult {
    SpeedtestOutboundResult {
        outbound_id: OutboundId::new(EngineId::new("fixture").unwrap(), id).unwrap(),
        display_name: Some(display_name.to_owned()),
        protocol_display_name: Some("Shadowsocks".to_owned()),
        samples,
        state,
        tested_at_epoch_ms: 1_700_000_000_000,
    }
}

fn snapshot() -> SpeedtestSnapshot {
    let mut snapshot = SpeedtestSnapshot::demo_fixture();
    snapshot.concurrency = SpeedtestConcurrency::new(12).unwrap();
    snapshot.results = vec![
            outbound_result(
                "proxy-a",
                "Node A",
                vec![
                    SpeedtestSampleOutcome::Measured { delay_ms: 40 },
                    SpeedtestSampleOutcome::TimedOut,
                    SpeedtestSampleOutcome::Measured { delay_ms: 44 },
                ],
                SpeedtestResultState::Completed,
            ),
            outbound_result(
                "proxy-b",
                "Node B",
                vec![
                    SpeedtestSampleOutcome::TimedOut,
                    SpeedtestSampleOutcome::TimedOut,
                ],
                SpeedtestResultState::TimedOut,
            ),
        ];
    snapshot.recent_history = vec![HistoricalSpeedtestRecord {
            run_id: SpeedtestRunId::new(5).unwrap(),
            timestamp_epoch_ms: 1_700_000_000_000,
            target: None,
            total_targets: 2,
            completed_targets: 2,
            avg_latency_ms: Some(42.0),
            avg_jitter_ms: Some(4.0),
        }];
    snapshot
}

#[test]
fn metrics_derive_from_preserved_samples_and_typed_result_identity() {
    let snapshot = snapshot();
    let first = &snapshot.results[0];
    let metrics = node_metrics(first);
    assert_eq!(first.outbound_id.value(), "proxy-a");
    assert_eq!(first.calculation().unwrap().sample_count, 3);
    assert!(metrics.jitter.ends_with("ms"));
    assert!(metrics.loss.ends_with('%'));
    let caption = metrics_label(Some(first), "en-US");
    assert!(caption.contains("Node A"));
}

#[test]
fn timeout_archive_and_history_use_explicit_typed_outcomes() {
    let snapshot = snapshot();
    let archive = dead_archive(&snapshot, "en-US");
    assert_eq!(archive.count, 1);
    assert_eq!(archive.names, "Node B");
    let lines = history_lines(&snapshot, "en-US");
    assert_eq!(lines.len(), 1);
    assert!(lines[0].contains("42.0 ms"));
    assert!(lines[0].contains("4.0 ms"));
}
