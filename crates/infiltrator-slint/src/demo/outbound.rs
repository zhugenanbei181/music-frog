//! The demo fixture's outbound inventory: the node/collection rows, their
//! per-node details and the UI preferences the proxies capture exercises.
//! Split out of `demo.rs` so the demo host keeps only window assembly.

use lurker_core_contract::identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, EngineId,
};
use lurker_core_contract::outbound::details::{
    OutboundDetail, OutboundDetailsSnapshot, OutboundDetailsState, OutboundEndpoint,
    OutboundFeature, OutboundHealthObservation, OutboundLatencySample, OutboundObservation,
    OutboundProbeAvailability, OutboundProtocolObservation, OutboundSnapshot, OutboundUdpSupport,
};
use lurker_core_contract::outbound::identity::{OutboundId, OutboundProtocolId, SnapshotRevision};
use lurker_core_contract::outbound::inventory::{
    OutboundInventorySnapshot, OutboundInventoryState, OutboundSelectionSet,
    OutboundSelectionState, SelectionAvailability,
};
use lurker_core_contract::outbound::preferences::{
    OutboundHealthFilter, OutboundLayout, OutboundSortOrder, OutboundUiPreferencesSnapshot,
    OutboundUiPreferencesValues,
};

pub(crate) fn demo_core_scope() -> CoreScope {
    CoreScope {
        identity: CoreIdentity {
            engine: EngineId::new("mihomo").expect("valid engine id"),
            instance: CoreInstanceId::new("demo-1").expect("valid instance id"),
            version: None,
        },
        generation: CoreGeneration::new(1),
        session_token: None,
    }
}

pub(crate) fn demo_outbound_inventory() -> (
    OutboundInventorySnapshot,
    OutboundDetailsSnapshot,
    OutboundUiPreferencesSnapshot,
) {
    let scope = demo_core_scope();
    let engine = scope.identity.engine.clone();
    let id = |value: &str| OutboundId::new(engine.clone(), value).expect("valid outbound id");

    let node_names = [
        ("HK-01", "HK IPLC 01", "shadowsocks", 38),
        ("HK-02", "HK BGP 02", "shadowsocks", 46),
        ("JP-01", "Tokyo 01", "vmess", 62),
        ("SG-01", "Singapore 01", "trojan", 75),
        ("US-01", "Los Angeles 01", "hysteria2", 158),
    ];

    let entries = node_names
        .iter()
        .map(|(nid, name, _, _)| OutboundSnapshot {
            id: id(nid),
            display_name: Some((*name).to_owned()),
        })
        .collect();

    let collection = |name: &str, members: &[&str], avail: SelectionAvailability, sel: &str| {
        OutboundSelectionSet {
            id: id(name),
            member_ids: members.iter().map(|m| id(m)).collect(),
            availability: avail,
            selection: OutboundSelectionState::Ready { selected: id(sel) },
        }
    };

    let inventory = OutboundInventorySnapshot::new(
        scope.clone(),
        SnapshotRevision::new(1),
        OutboundInventoryState::Ready {
            entries,
            collections: vec![
                collection(
                    "PROXIES",
                    &["HK-01", "HK-02", "JP-01", "SG-01", "US-01"],
                    SelectionAvailability::Selectable,
                    "HK-01",
                ),
                collection(
                    "AUTO",
                    &["HK-01", "HK-02", "JP-01"],
                    SelectionAvailability::Automatic,
                    "HK-01",
                ),
                // The global group is the one the published exit names, so the
                // proxies page opens on the group the owner marked as active.
                collection(
                    "GLOBAL",
                    &["HK-01", "HK-02", "JP-01", "SG-01", "US-01"],
                    SelectionAvailability::Selectable,
                    "HK-01",
                ),
            ],
        },
    )
    .expect("valid demo inventory");

    let details = node_names
        .iter()
        .map(|(nid, _name, proto, delay)| OutboundDetail {
            id: id(nid),
            protocol: OutboundProtocolObservation::Reported(
                OutboundProtocolId::new(engine.clone(), *proto).expect("valid proto"),
            ),
            protocol_display_name: Some(proto.to_uppercase()),
            cipher: OutboundObservation::Unknown,
            cipher_display_name: None,
            collection_kind: OutboundObservation::Unknown,
            features: OutboundObservation::Observed(vec![OutboundFeature::Udp]),
            endpoint: OutboundObservation::Observed(OutboundEndpoint {
                host: format!("{nid}.node.example.com"),
                port: OutboundObservation::Observed(443),
            }),
            udp: OutboundUdpSupport::Supported,
            health: OutboundHealthObservation::Healthy,
            current_latency_ms: OutboundObservation::Observed(*delay),
            latency_history: OutboundObservation::Observed(vec![OutboundLatencySample {
                observed_at: "2026-10-16T12:00:00Z".to_owned(),
                delay_ms: *delay,
            }]),
            rtt: OutboundObservation::Unknown,
            historical_egress: OutboundObservation::Unknown,
            probe_availability: OutboundProbeAvailability::Supported,
        })
        .collect();

    let details_snapshot = OutboundDetailsSnapshot::new(
        scope.clone(),
        SnapshotRevision::new(1),
        OutboundDetailsState::Ready { details },
    )
    .expect("valid demo details");

    let preferences = OutboundUiPreferencesSnapshot::new(
        scope,
        SnapshotRevision::new(1),
        OutboundUiPreferencesValues {
            search_query: String::new(),
            favorite_outbound_ids: Vec::new(),
            group_order: vec![id("PROXIES"), id("AUTO")],
            collapsed_group_ids: Vec::new(),
            sort_order: OutboundSortOrder::LatencyAscending,
            health_filter: OutboundHealthFilter::All,
            layout: OutboundLayout::CompactList,
        },
    )
    .expect("valid demo preferences");

    (inventory, details_snapshot, preferences)
}
