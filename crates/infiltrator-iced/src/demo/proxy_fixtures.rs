//! Typed outbound observations used by explicit Iced demo captures.

use infiltrator_contract::fixture_identities::{
    AI, AUTO, CAMPUS, DIRECT_GROUP, GAMING, HK_PRIMARY, HK_SECONDARY, JP, PROXIES, SG, US,
};
use infiltrator_contract::resources::{
    CORE_MEMORY_SOFT_LIMIT_BYTES, CoreGcStatus, CoreResourceSnapshot, ResourceAvailability,
};
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use lurker_core_contract::error::Failure;
use lurker_core_contract::identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, CoreVersion, EngineId,
};
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_contract::outbound::details::OutboundFeature;
use lurker_core_contract::outbound::{
    details::{
        OutboundCollectionKind, OutboundDetail, OutboundDetailsSnapshot, OutboundDetailsState,
        OutboundEndpoint, OutboundHealthObservation, OutboundLatencySample, OutboundObservation,
        OutboundProbeAvailability, OutboundProtocolObservation, OutboundSnapshot,
        OutboundUdpSupport,
    },
    identity::{OutboundCipherId, OutboundId, OutboundProtocolId, SnapshotRevision},
    inventory::{
        OutboundInventorySnapshot, OutboundInventoryState, OutboundSelectionSet,
        OutboundSelectionState, SelectionAvailability,
    },
    preferences::{
        OutboundHealthFilter, OutboundLayout, OutboundSortOrder, OutboundUiPreferencesSnapshot,
        OutboundUiPreferencesValues,
    },
};
use lurker_core_contract::page::{PageData, SurfaceOrigin};
use lurker_core_contract::routing_intent::RoutingIntentSnapshot;
use lurker_core_contract::session::SessionToken;

const GLOBAL: &str = "GLOBAL";
const DMIT: &str = "DMIT";
const ZGO: &str = "ZGO";
const DIRECT: &str = "DIRECT";
const REJECT: &str = "REJECT";

#[derive(Clone, Copy)]
struct MemberFixture {
    name: &'static str,
    protocol: &'static str,
    protocol_label: &'static str,
    cipher: Option<&'static str>,
    cipher_label: Option<&'static str>,
    host: Option<&'static str>,
    port: Option<u16>,
    latency_ms: Option<u32>,
}

const MEMBERS: [MemberFixture; 9] = [
    MemberFixture {
        name: HK_PRIMARY,
        protocol: "shadowsocks",
        protocol_label: "Shadowsocks",
        cipher: Some("aes-256-gcm"),
        cipher_label: Some("AES-256-GCM"),
        host: Some("hk-iepl-01.example.com"),
        port: Some(8443),
        latency_ms: Some(148),
    },
    MemberFixture {
        name: DMIT,
        protocol: "vmess",
        protocol_label: "VMess",
        cipher: Some("auto"),
        cipher_label: Some("auto"),
        host: Some("dmit.example.com"),
        port: Some(443),
        latency_ms: Some(189),
    },
    MemberFixture {
        name: HK_SECONDARY,
        protocol: "shadowsocks",
        protocol_label: "Shadowsocks",
        cipher: Some("aes-256-gcm"),
        cipher_label: Some("AES-256-GCM"),
        host: Some("hk-iepl-02.example.com"),
        port: Some(8443),
        latency_ms: Some(233),
    },
    MemberFixture {
        name: SG,
        protocol: "trojan",
        protocol_label: "Trojan",
        cipher: None,
        cipher_label: None,
        host: Some("sg-bgp.example.com"),
        port: Some(443),
        latency_ms: Some(254),
    },
    MemberFixture {
        name: JP,
        protocol: "vmess",
        protocol_label: "VMess",
        cipher: Some("auto"),
        cipher_label: Some("auto"),
        host: Some("jp-ntt.example.com"),
        port: Some(443),
        latency_ms: Some(312),
    },
    MemberFixture {
        name: US,
        protocol: "trojan",
        protocol_label: "Trojan",
        cipher: None,
        cipher_label: None,
        host: Some("us-cn2.example.com"),
        port: Some(443),
        latency_ms: Some(512),
    },
    MemberFixture {
        name: ZGO,
        protocol: "hysteria2",
        protocol_label: "Hysteria2",
        cipher: None,
        cipher_label: None,
        host: Some("zgo.example.com"),
        port: Some(36712),
        latency_ms: None,
    },
    MemberFixture {
        name: DIRECT,
        protocol: "direct",
        protocol_label: "Direct",
        cipher: None,
        cipher_label: None,
        host: None,
        port: None,
        latency_ms: None,
    },
    MemberFixture {
        name: REJECT,
        protocol: "reject",
        protocol_label: "Reject",
        cipher: None,
        cipher_label: None,
        host: None,
        port: None,
        latency_ms: None,
    },
];

struct CollectionFixture {
    id: &'static str,
    selected: &'static str,
    kind: OutboundCollectionKind,
    availability: SelectionAvailability,
    members: &'static [&'static str],
}

const COLLECTIONS: [CollectionFixture; 7] = [
    CollectionFixture {
        id: PROXIES,
        selected: HK_PRIMARY,
        kind: OutboundCollectionKind::ManualSelection,
        availability: SelectionAvailability::Selectable,
        members: &[HK_PRIMARY, DMIT, HK_SECONDARY, SG, JP, US, ZGO],
    },
    CollectionFixture {
        id: DIRECT_GROUP,
        selected: DIRECT,
        kind: OutboundCollectionKind::ManualSelection,
        availability: SelectionAvailability::Selectable,
        members: &[DIRECT, HK_PRIMARY, DMIT],
    },
    CollectionFixture {
        id: CAMPUS,
        selected: DIRECT,
        kind: OutboundCollectionKind::ManualSelection,
        availability: SelectionAvailability::Selectable,
        members: &[DIRECT, REJECT, HK_SECONDARY, DMIT],
    },
    CollectionFixture {
        id: AI,
        selected: US,
        kind: OutboundCollectionKind::ManualSelection,
        availability: SelectionAvailability::Selectable,
        members: &[US, SG, JP, DMIT],
    },
    CollectionFixture {
        id: GAMING,
        selected: JP,
        kind: OutboundCollectionKind::ManualSelection,
        availability: SelectionAvailability::Selectable,
        members: &[JP, HK_PRIMARY, HK_SECONDARY, US],
    },
    CollectionFixture {
        id: AUTO,
        selected: HK_PRIMARY,
        kind: OutboundCollectionKind::AutomaticSelection,
        availability: SelectionAvailability::Automatic,
        members: &[HK_PRIMARY, DMIT, HK_SECONDARY, SG, JP],
    },
    CollectionFixture {
        id: GLOBAL,
        selected: PROXIES,
        kind: OutboundCollectionKind::ManualSelection,
        availability: SelectionAvailability::Selectable,
        members: &[PROXIES, DIRECT_GROUP, CAMPUS, AI, GAMING, AUTO],
    },
];

/// A fully explicit demo surface with typed inventory, details and preferences.
/// Core capability evidence stays unavailable, so demo data never enables a
/// product probe command.
pub(super) fn demo_outbound_surface() -> SurfaceSnapshot {
    let engine = EngineId::new("mihomo").expect("valid fixture engine");
    let scope = CoreScope {
        identity: CoreIdentity::new(
            engine.clone(),
            CoreInstanceId::new("iced-demo").expect("valid fixture core instance"),
            Some(CoreVersion::new("1.19.32").expect("valid fixture core version")),
        ),
        generation: CoreGeneration::new(1),
        session_token: Some(SessionToken::new(1)),
    };
    let id = |value: &str| OutboundId::new(engine.clone(), value).expect("valid fixture outbound");
    let group_ids = COLLECTIONS
        .iter()
        .map(|group| id(group.id))
        .collect::<Vec<_>>();
    let mut entries = group_ids
        .iter()
        .map(|group_id| OutboundSnapshot {
            id: group_id.clone(),
            display_name: Some(group_id.value().to_owned()),
        })
        .collect::<Vec<_>>();
    entries.extend(MEMBERS.iter().map(|member| OutboundSnapshot {
        id: id(member.name),
        display_name: Some(member.name.to_owned()),
    }));
    let collections = COLLECTIONS
        .iter()
        .map(|group| {
            let selected = id(group.selected);
            OutboundSelectionSet {
                id: id(group.id),
                member_ids: group.members.iter().map(|member| id(member)).collect(),
                availability: group.availability,
                selection: OutboundSelectionState::Ready { selected },
            }
        })
        .collect::<Vec<_>>();
    let inventory = OutboundInventorySnapshot::new(
        scope.clone(),
        SnapshotRevision::new(1),
        OutboundInventoryState::Ready {
            entries,
            collections,
        },
    )
    .expect("valid demo outbound inventory");

    let mut details = COLLECTIONS
        .iter()
        .map(|group| OutboundDetail {
            id: id(group.id),
            protocol: OutboundProtocolObservation::Unknown,
            protocol_display_name: None,
            cipher: OutboundObservation::Unsupported,
            cipher_display_name: None,
            collection_kind: OutboundObservation::Observed(group.kind),
            features: OutboundObservation::Unknown,
            endpoint: OutboundObservation::Unsupported,
            udp: OutboundUdpSupport::Unsupported,
            health: OutboundHealthObservation::Unsupported,
            current_latency_ms: OutboundObservation::Unsupported,
            latency_history: OutboundObservation::Unsupported,
            rtt: OutboundObservation::Unsupported,
            historical_egress: OutboundObservation::Unsupported,
            probe_availability: OutboundProbeAvailability::Unknown,
        })
        .collect::<Vec<_>>();
    details.extend(MEMBERS.iter().map(|member| {
        let current_latency_ms = member
            .latency_ms
            .map(OutboundObservation::Observed)
            .unwrap_or(OutboundObservation::Unknown);
        let latency_history = OutboundObservation::Observed(
            member
                .latency_ms
                .map(|delay_ms| {
                    vec![OutboundLatencySample {
                        observed_at: "2026-08-29T15:31:00.000000+08:00".to_owned(),
                        delay_ms,
                    }]
                })
                .unwrap_or_default(),
        );
        OutboundDetail {
            id: id(member.name),
            protocol: OutboundProtocolObservation::Reported(
                OutboundProtocolId::new(engine.clone(), member.protocol)
                    .expect("valid fixture protocol"),
            ),
            protocol_display_name: Some(member.protocol_label.to_owned()),
            cipher: member
                .cipher
                .map_or(OutboundObservation::Unsupported, |cipher| {
                    OutboundObservation::Observed(
                        OutboundCipherId::new(engine.clone(), cipher)
                            .expect("valid fixture cipher"),
                    )
                }),
            cipher_display_name: member.cipher_label.map(str::to_owned),
            collection_kind: OutboundObservation::Unknown,
            features: OutboundObservation::Observed(vec![OutboundFeature::Udp]),
            endpoint: member
                .host
                .map_or(OutboundObservation::Unsupported, |host| {
                    OutboundObservation::Observed(OutboundEndpoint {
                        host: host.to_owned(),
                        port: member
                            .port
                            .map_or(OutboundObservation::Unknown, OutboundObservation::Observed),
                    })
                }),
            udp: OutboundUdpSupport::Supported,
            health: OutboundHealthObservation::Healthy,
            current_latency_ms,
            latency_history,
            rtt: OutboundObservation::Unknown,
            historical_egress: OutboundObservation::Unknown,
            probe_availability: OutboundProbeAvailability::Unknown,
        }
    }));
    let details = OutboundDetailsSnapshot::new(
        scope.clone(),
        SnapshotRevision::new(1),
        OutboundDetailsState::Ready { details },
    )
    .expect("valid demo outbound details");
    let preferences = OutboundUiPreferencesSnapshot::new(
        scope.clone(),
        SnapshotRevision::new(1),
        OutboundUiPreferencesValues {
            search_query: String::new(),
            favorite_outbound_ids: Vec::new(),
            group_order: group_ids,
            // The first group starts folded: the fold business has to open it
            // for real, and the route-only render it is measured against shows
            // the folded group.
            collapsed_group_ids: vec![id(PROXIES)],
            sort_order: OutboundSortOrder::LatencyAscending,
            health_filter: OutboundHealthFilter::All,
            layout: OutboundLayout::CompactList,
        },
    )
    .expect("valid demo outbound preferences");

    let mut snapshot = SurfaceSnapshot::unavailable(
        SurfaceKind::IcedDesktop,
        HostKind::Desktop,
        Failure::unsupported("demo surface uses explicit fixtures"),
    );
    snapshot.origin = SurfaceOrigin::Demo;
    snapshot.generation = 1;
    snapshot.revision = 1;
    snapshot.core.identity = Some(scope.identity.clone());
    snapshot.core.generation = 1;
    snapshot.core.session_token = scope.session_token;
    snapshot.core.lifecycle = CoreLifecycle::Stopped;
    snapshot.core.routing_intent = RoutingIntentSnapshot::demo_fixture();
    snapshot.core.core_version = Some("1.19.32".to_owned());
    snapshot.resources = CoreResourceSnapshot::Observed {
        scope: Box::new(scope),
        memory: ResourceAvailability::Available(96_468_992),
        cpu_percent: ResourceAvailability::Available(2.4),
        memory_soft_limit_bytes: CORE_MEMORY_SOFT_LIMIT_BYTES,
        gc: Box::new(CoreGcStatus::NotAttempted),
    };
    snapshot.outbound_inventory = PageData::ready(inventory);
    snapshot.outbound_details = PageData::ready(details);
    snapshot.outbound_ui_preferences = PageData::ready(preferences);
    snapshot
}
