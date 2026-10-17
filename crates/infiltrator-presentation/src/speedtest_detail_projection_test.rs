use super::{listing, project_details};
use infiltrator_contract::core_identity::EngineId;
use infiltrator_contract::error::Failure;
use infiltrator_contract::outbound::identity::OutboundId;
use infiltrator_contract::speedtest::{
    SpeedtestOutboundResult, SpeedtestResultState, SpeedtestSampleOutcome, SpeedtestSnapshot,
};

fn result(id: &str, name: &str, delay: u32) -> SpeedtestOutboundResult {
    SpeedtestOutboundResult {
        outbound_id: OutboundId::new(EngineId::new("fixture").unwrap(), id).unwrap(),
        display_name: Some(name.to_owned()),
        protocol_display_name: Some("VLESS".to_owned()),
        samples: vec![
            SpeedtestSampleOutcome::Measured { delay_ms: delay },
            SpeedtestSampleOutcome::Measured {
                delay_ms: delay + 4,
            },
        ],
        state: SpeedtestResultState::Completed,
        tested_at_epoch_ms: 1,
    }
}

#[test]
fn detail_rows_keep_outbound_ids_and_all_jitter_samples_in_stable_latency_order() {
    let mut snapshot = SpeedtestSnapshot::demo_fixture();
    snapshot.results = vec![result("second", "Second", 35), result("first", "First", 20)];

    let details = project_details(&snapshot);

    assert_eq!(details.rows[0].outbound_id.value(), "first");
    assert_eq!(details.rows[0].sample_count, 2);
    assert_eq!(details.rows[0].delay, Some(20));
    assert_eq!(details.rows[0].jitter_ms, Some(4.0));
    assert!(listing(&details, &str::to_owned).contains("First · VLESS"));
    assert!(listing(&details, &str::to_owned).contains("2"));
}

#[test]
fn unavailable_empty_detail_snapshot_stays_empty_and_explicit() {
    let snapshot =
        SpeedtestSnapshot::unavailable(Failure::unsupported("outbound owner is unavailable"));

    let details = project_details(&snapshot);
    assert!(details.rows.is_empty());
    assert_eq!(details.phase, snapshot.phase());
    assert_eq!(
        listing(&details, &str::to_owned),
        "speedtest_phase_unavailable: outbound owner is unavailable"
    );
}
