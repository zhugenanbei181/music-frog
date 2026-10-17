//! Typed outbound observations used only by explicit interaction-capture scenarios.
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use lurker_core_contract::identity::EngineId;
use lurker_core_contract::outbound::{
    details::{
        OutboundCollectionKind, OutboundDetail, OutboundDetailsSnapshot, OutboundDetailsState,
        OutboundEgressSample, OutboundEndpoint, OutboundFeature, OutboundHealthObservation,
        OutboundLatencySample, OutboundObservation, OutboundProbeAvailability,
        OutboundProtocolObservation, OutboundRttStatistics, OutboundSnapshot, OutboundUdpSupport,
    },
    identity::{OutboundCipherId, OutboundId, OutboundProtocolId, SnapshotRevision},
    inventory::{
        OutboundInventorySnapshot, OutboundInventoryState, OutboundSelectionSet,
        OutboundSelectionState, SelectionAvailability,
    },
};
use lurker_core_contract::page::PageData;
use std::collections::HashSet;

pub(super) const INSPECTION_NODE: &str = "Inspection node";
pub(super) const INSPECTION_GROUP: &str = "Inspection group";
pub(super) const SEARCH_QUERY: &str = "HK-01";
pub(super) const SEARCH_GROUP: &str = "Search policy";
pub(super) const FIRST_GROUP: &str = "A-Preferred";
pub(super) const MOVED_GROUP: &str = "B-Streaming";
pub(super) const LAST_GROUP: &str = "C-Fallback";

pub(super) fn set_capture_inventory(
    snapshot: &mut SurfaceSnapshot,
    collections: &[(&str, &[&str], SelectionAvailability)],
) {
    let scope = snapshot
        .core
        .scope()
        .expect("capture surface provides a typed active core scope");
    let engine = scope.identity.engine.clone();
    let mut entries = Vec::new();
    let mut outbound_ids = HashSet::new();
    let mut sets = Vec::with_capacity(collections.len());
    for (collection, members, availability) in collections {
        let member_ids: Vec<_> = members
            .iter()
            .map(|member| {
                let id =
                    OutboundId::new(engine.clone(), *member).expect("capture outbound ID is valid");
                if outbound_ids.insert(id.clone()) {
                    entries.push(OutboundSnapshot {
                        id: id.clone(),
                        display_name: Some((*member).to_owned()),
                    });
                }
                id
            })
            .collect();
        let selection = member_ids
            .first()
            .cloned()
            .map_or(OutboundSelectionState::Unknown, |selected| {
                OutboundSelectionState::Ready { selected }
            });
        sets.push(OutboundSelectionSet {
            id: OutboundId::new(engine.clone(), *collection)
                .expect("capture collection ID is valid"),
            member_ids,
            availability: *availability,
            selection,
        });
    }
    let revision = snapshot
        .outbound_inventory
        .data
        .as_ref()
        .map_or(1, |inventory| {
            inventory.revision().value().saturating_add(1)
        });
    let inventory = OutboundInventorySnapshot::new(
        scope,
        SnapshotRevision::new(revision),
        OutboundInventoryState::Ready {
            entries,
            collections: sets,
        },
    )
    .expect("capture outbound inventory is valid");
    snapshot.outbound_inventory = PageData::ready(inventory);
}

pub(super) fn set_inspection_observations(snapshot: &mut SurfaceSnapshot) {
    set_capture_inventory(
        snapshot,
        &[(
            INSPECTION_GROUP,
            &[INSPECTION_NODE],
            SelectionAvailability::Selectable,
        )],
    );
    let scope = snapshot
        .core
        .scope()
        .expect("capture surface provides a typed active core scope");
    let detail_revision = snapshot
        .outbound_details
        .data
        .as_ref()
        .map_or(1, |details| {
            details.detail_revision().value().saturating_add(1)
        });
    let details = OutboundDetailsSnapshot::new(
        scope.clone(),
        SnapshotRevision::new(detail_revision),
        OutboundDetailsState::Ready {
            details: vec![inspection_detail(&scope.identity.engine)],
        },
    )
    .expect("capture outbound details are valid");
    snapshot.outbound_details = PageData::ready(details);
}

fn inspection_detail(engine: &EngineId) -> OutboundDetail {
    OutboundDetail {
        id: OutboundId::new(engine.clone(), INSPECTION_NODE).expect("capture outbound ID is valid"),
        protocol: OutboundProtocolObservation::Reported(
            OutboundProtocolId::new(engine.clone(), "shadowsocks")
                .expect("capture protocol ID is valid"),
        ),
        protocol_display_name: Some("Shadowsocks".to_owned()),
        cipher: OutboundObservation::Observed(
            OutboundCipherId::new(engine.clone(), "aes-256-gcm")
                .expect("capture cipher ID is valid"),
        ),
        cipher_display_name: Some("AES-256-GCM".to_owned()),
        collection_kind: OutboundObservation::Observed(OutboundCollectionKind::ManualSelection),
        features: OutboundObservation::Observed(vec![OutboundFeature::Udp]),
        endpoint: OutboundObservation::Observed(OutboundEndpoint {
            host: "node.example.test".to_owned(),
            port: OutboundObservation::Observed(443),
        }),
        udp: OutboundUdpSupport::Supported,
        health: OutboundHealthObservation::Healthy,
        current_latency_ms: OutboundObservation::Observed(42),
        latency_history: OutboundObservation::Observed(vec![
            OutboundLatencySample {
                observed_at: "2026-10-05T00:00:00Z".to_owned(),
                delay_ms: 18,
            },
            OutboundLatencySample {
                observed_at: "2026-10-05T00:01:00Z".to_owned(),
                delay_ms: 0,
            },
            OutboundLatencySample {
                observed_at: "2026-10-05T00:02:00Z".to_owned(),
                delay_ms: 42,
            },
        ]),
        rtt: OutboundObservation::Observed(OutboundRttStatistics {
            min_ms: Some(18),
            max_ms: Some(42),
            avg_ms: Some(30),
            valid_count: 2,
        }),
        historical_egress: OutboundObservation::Observed(vec![OutboundEgressSample {
            ip: "203.0.113.8".to_owned(),
            country: Some("JP".to_owned()),
            observed_at_epoch_ms: Some(1_760_000_000_000),
        }]),
        probe_availability: OutboundProbeAvailability::Unsupported,
    }
}
