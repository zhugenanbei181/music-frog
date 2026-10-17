//! Tests for Outbound proxy selection, probing/speedtest, sorting, and search preferences in infiltrator-slint.

use infiltrator_contract::command::CommandIntent;
use lurker_core_contract::identity::EngineId;
use lurker_core_contract::outbound::identity::{OutboundId, SnapshotRevision};
use lurker_core_contract::outbound::preferences::{
    OutboundSortOrder, OutboundUiPreferenceIntent, OutboundUiPreferenceRequest,
};
use lurker_core_contract::outbound::probe::{
    OutboundProbeBatchScope, OutboundProbeConcurrency, OutboundSelectionRequest,
    ProbeOutboundBatchRequest, ProbeOutboundRequest,
};

#[test]
fn test_proxy_selection_request_pins_inventory_revision() {
    let engine = EngineId::mihomo();
    let group = OutboundId::new(engine.clone(), "PROXIES").unwrap();
    let node = OutboundId::new(engine, "SGP-01").unwrap();
    let revision = SnapshotRevision::new(10);

    let request = OutboundSelectionRequest::new(group.clone(), node.clone(), revision).unwrap();
    assert_eq!(request.collection_id(), &group);
    assert_eq!(request.outbound_id(), &node);
    assert_eq!(request.expected_revision(), revision);

    let intent = CommandIntent::SelectOutbound { request };
    match intent {
        CommandIntent::SelectOutbound { request } => {
            assert_eq!(request.expected_revision().value(), 10);
            assert_eq!(request.collection_id().value(), "PROXIES");
            assert_eq!(request.outbound_id().value(), "SGP-01");
        }
        _ => panic!("wrong intent"),
    }
}

#[test]
fn test_single_node_probe_request_carries_probe_settings() {
    let engine = EngineId::mihomo();
    let node = OutboundId::new(engine, "HKG-01").unwrap();
    let revision = SnapshotRevision::new(5);
    let url = "http://cp.cloudflare.com/generate_204";
    let timeout_ms = 5000u64;

    let request = ProbeOutboundRequest::new(node.clone(), revision, url, timeout_ms).unwrap();

    assert_eq!(request.outbound_id(), &node);
    assert_eq!(request.test_url(), url);
    assert_eq!(request.timeout_ms(), timeout_ms);
}

#[test]
fn test_batch_group_probe_request_uses_collection_scope_and_concurrency_8() {
    let engine = EngineId::mihomo();
    let group = OutboundId::new(engine, "PROXIES").unwrap();
    let revision = SnapshotRevision::new(5);
    let url = "http://cp.cloudflare.com/generate_204";
    let timeout_ms = 5000u64;

    let concurrency = OutboundProbeConcurrency::new(8).unwrap();
    let request = ProbeOutboundBatchRequest::new(
        OutboundProbeBatchScope::Collection(group.clone()),
        revision,
        url,
        timeout_ms,
        concurrency,
    )
    .unwrap();

    assert_eq!(
        request.target_scope(),
        &OutboundProbeBatchScope::Collection(group)
    );
    assert_eq!(request.concurrency().limit(), 8);
    assert_eq!(request.timeout_ms(), timeout_ms);
}

#[test]
fn test_outbound_ui_preference_request_intents() {
    let revision = SnapshotRevision::new(1);

    let search_req = OutboundUiPreferenceRequest::new(
        revision,
        OutboundUiPreferenceIntent::SetSearchQuery("HKG".into()),
    )
    .unwrap();
    assert_eq!(
        search_req.intent(),
        &OutboundUiPreferenceIntent::SetSearchQuery("HKG".into())
    );

    let sort_req = OutboundUiPreferenceRequest::new(
        revision,
        OutboundUiPreferenceIntent::SetSortOrder(OutboundSortOrder::NameDescending),
    )
    .unwrap();
    assert_eq!(
        sort_req.intent(),
        &OutboundUiPreferenceIntent::SetSortOrder(OutboundSortOrder::NameDescending)
    );
}
