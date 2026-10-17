use super::details::{
    OutboundCollectionKind, OutboundDetail, OutboundDetailsSnapshot, OutboundDetailsState,
    OutboundEndpoint, OutboundFeature, OutboundHealthObservation, OutboundLatencySample,
    OutboundObservation, OutboundProbeAvailability, OutboundProtocolObservation,
    OutboundRttStatistics, OutboundSnapshot, OutboundUdpSupport,
};
use super::identity::{OutboundContractError, OutboundId, OutboundProtocolId, SnapshotRevision};
use super::inventory::{
    OutboundInventorySnapshot, OutboundInventoryState, OutboundObservationSnapshot,
    OutboundProbeSnapshot, OutboundProbeState, OutboundSelectionSet, OutboundSelectionState,
    SelectionAvailability,
};
use super::preferences::{
    OutboundFavoriteState, OutboundGroupExpansion, OutboundHealthFilter, OutboundLayout,
    OutboundSortOrder, OutboundUiPreferenceIntent, OutboundUiPreferenceRequest,
    OutboundUiPreferencesSnapshot, OutboundUiPreferencesValues,
};
use super::probe::{
    MAX_OUTBOUND_PROBE_BATCH_CONCURRENCY, OutboundProbeBatchItem, OutboundProbeBatchOutcome,
    OutboundProbeBatchScope, OutboundProbeBatchSnapshot, OutboundProbeConcurrency,
    OutboundSelectionRequest, ProbeOutboundBatchRequest, ProbeOutboundRequest,
};
use crate::core_identity::{CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, EngineId};
use crate::error::{ErrorCode, Failure};
use serde_json::{Value, json};

fn scope(engine: &str) -> CoreScope {
    CoreScope {
        identity: CoreIdentity::new(
            EngineId::new(engine).expect("valid engine ID"),
            CoreInstanceId::new("primary").expect("valid instance ID"),
            None,
        ),
        generation: CoreGeneration::new(9),
        session_token: None,
    }
}

fn outbound(engine: &str, value: &str) -> OutboundSnapshot {
    OutboundSnapshot {
        id: outbound_id(engine, value),
        display_name: None,
    }
}

fn outbound_id(engine: &str, value: &str) -> OutboundId {
    OutboundId::new(EngineId::new(engine).expect("valid engine ID"), value)
        .expect("valid outbound ID")
}

fn selection_set(
    engine: &str,
    id: &str,
    members: &[&str],
    availability: SelectionAvailability,
    selection: OutboundSelectionState,
) -> OutboundSelectionSet {
    OutboundSelectionSet {
        id: outbound_id(engine, id),
        member_ids: members
            .iter()
            .map(|member| outbound_id(engine, member))
            .collect(),
        availability,
        selection,
    }
}

fn valid_mihomo_snapshot() -> OutboundInventorySnapshot {
    OutboundInventorySnapshot::new(
        scope("mihomo"),
        SnapshotRevision::new(4),
        OutboundInventoryState::Ready {
            entries: vec![
                outbound("mihomo", "node-a"),
                outbound("mihomo", "node-b"),
                outbound("mihomo", "node-c"),
                outbound("mihomo", "node-d"),
            ],
            collections: vec![
                selection_set(
                    "mihomo",
                    "collection/primary",
                    &["node-a", "node-b"],
                    SelectionAvailability::Selectable,
                    OutboundSelectionState::Ready {
                        selected: outbound_id("mihomo", "node-a"),
                    },
                ),
                selection_set(
                    "mihomo",
                    "collection/streaming",
                    &["node-c", "node-d"],
                    SelectionAvailability::ReadOnly,
                    OutboundSelectionState::Ready {
                        selected: outbound_id("mihomo", "node-c"),
                    },
                ),
            ],
        },
    )
    .expect("valid Mihomo-style inventory")
}

fn detail(engine: &str, outbound: &str) -> OutboundDetail {
    let id = outbound_id(engine, outbound);
    OutboundDetail {
        protocol: OutboundProtocolObservation::Reported(
            OutboundProtocolId::new(id.engine_id().clone(), "direct").expect("protocol identity"),
        ),
        protocol_display_name: Some("Direct".to_owned()),
        cipher: OutboundObservation::Unsupported,
        cipher_display_name: None,
        collection_kind: OutboundObservation::Unsupported,
        features: OutboundObservation::Observed(vec![OutboundFeature::Udp]),
        endpoint: OutboundObservation::Observed(OutboundEndpoint {
            host: "example.test".to_owned(),
            port: OutboundObservation::Unknown,
        }),
        udp: OutboundUdpSupport::Supported,
        health: OutboundHealthObservation::Healthy,
        current_latency_ms: OutboundObservation::Observed(0),
        latency_history: OutboundObservation::Observed(vec![OutboundLatencySample {
            observed_at: "2026-10-10T00:00:00Z".to_owned(),
            delay_ms: 0,
        }]),
        rtt: OutboundObservation::Observed(OutboundRttStatistics::default()),
        historical_egress: OutboundObservation::Observed(vec![]),
        probe_availability: OutboundProbeAvailability::Supported,
        id,
    }
}

#[path = "outbound_tests/details_preferences.rs"]
mod details_preferences;
#[path = "outbound_tests/identity_probe.rs"]
mod identity_probe;
#[path = "outbound_tests/inventory.rs"]
mod inventory;
