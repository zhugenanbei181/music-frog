//! SLINT-001.8: The proxies page and the overview page of the real Slint
//! surface read the shared page models.
//!
//! Every case drives the production Slint projection through the headless
//! surface harness. The query rule, the member order and the traffic mode
//! belong to the shared page models, so the cases below prove the shared
//! behaviour instead of a surface-local copy of it: the protocol alias case
//! and the pinyin case cannot pass with the plain substring search the
//! surface used to run, and the traffic mode case cannot pass with a read of
//! the configured TUN value.

use infiltrator_contract::runtime_control::{RuntimeControlSnapshot, RuntimeControlStatus};
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::{SettingsPageSnapshot, SurfaceSnapshot};
use infiltrator_contract::system_proxy::SystemProxyStatus;
use infiltrator_slint::surface::harness;
use infiltrator_slint::widget_vocabulary;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, EngineId,
};
use lurker_core_contract::outbound::details::{
    OutboundDetail, OutboundDetailsSnapshot, OutboundDetailsState, OutboundFeature,
    OutboundHealthObservation, OutboundObservation, OutboundProbeAvailability,
    OutboundProtocolObservation, OutboundSnapshot, OutboundUdpSupport,
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
use lurker_core_contract::page::PageData;
use lurker_core_contract::routing_intent::{
    RoutingIntent, RoutingIntentSnapshot, RoutingIntentStatus,
};

/// One node the owner publishes.
struct Node {
    id: &'static str,
    name: &'static str,
    protocol: &'static str,
    latency: Option<u32>,
    /// Whether the backend reported that the node carries UDP traffic.
    udp: bool,
    /// Whether the backend advertised the UDP feature tag.
    tags_udp: bool,
    healthy: bool,
}

/// The nodes the owner publishes.
///
/// The set carries one node per shape the shared rule answers differently: a
/// node whose name is reachable only through its pinyin initials, a node
/// whose protocol is reachable only through the `ss` alias, a node that
/// carries UDP traffic without advertising the tag, a node without a
/// measured latency and a node the last probe reported as unhealthy.
///
/// The protocol spellings carry no `ss` of their own: the shared rule
/// matches a protocol spelling as a substring, so `vless` and `vmess` would
/// answer the `ss` query on their own and the alias case would prove
/// nothing.
const NODES: [Node; 4] = [
    Node {
        id: "HK-01",
        name: "香港 01",
        protocol: "trojan",
        latency: Some(45),
        udp: true,
        tags_udp: true,
        healthy: true,
    },
    Node {
        id: "JP-01",
        name: "Tokyo 01",
        protocol: "shadowsocks",
        latency: Some(120),
        udp: true,
        tags_udp: false,
        healthy: true,
    },
    Node {
        id: "JP-02",
        name: "Osaka 01",
        protocol: "hysteria2",
        latency: None,
        udp: false,
        tags_udp: false,
        healthy: true,
    },
    Node {
        id: "DE-01",
        name: "Berlin 01",
        protocol: "tuic",
        latency: Some(90),
        udp: false,
        tags_udp: false,
        healthy: false,
    },
];

fn scope() -> CoreScope {
    CoreScope {
        identity: CoreIdentity::new(
            EngineId::mihomo(),
            CoreInstanceId::new("slint-page-model-test").unwrap(),
            None,
        ),
        generation: CoreGeneration::new(1),
        session_token: None,
    }
}

fn outbound_id(value: &str) -> OutboundId {
    OutboundId::new(EngineId::mihomo(), value).unwrap()
}

/// The owner snapshot the proxies page reads.
fn proxy_snapshot(query: &str, sort: OutboundSortOrder, favorites: &[&str]) -> SurfaceSnapshot {
    let mut snapshot = SurfaceSnapshot::unavailable(
        SurfaceKind::SlintDesktop,
        HostKind::Desktop,
        Failure::new(ErrorCode::NotReady, "initial", true),
    );
    let engine = EngineId::mihomo();
    let entries = NODES
        .iter()
        .map(|node| OutboundSnapshot {
            id: outbound_id(node.id),
            display_name: Some(node.name.to_owned()),
        })
        .collect();
    let inventory = OutboundInventorySnapshot::new(
        scope(),
        SnapshotRevision::new(10),
        OutboundInventoryState::Ready {
            entries,
            collections: vec![OutboundSelectionSet {
                id: outbound_id("PROXIES"),
                member_ids: NODES.iter().map(|node| outbound_id(node.id)).collect(),
                availability: SelectionAvailability::Selectable,
                selection: OutboundSelectionState::Ready {
                    selected: outbound_id("HK-01"),
                },
            }],
        },
    )
    .expect("valid engine-scoped inventory");
    snapshot.outbound_inventory = PageData::ready(inventory);
    let details = NODES
        .iter()
        .map(|node| OutboundDetail {
            id: outbound_id(node.id),
            protocol: OutboundProtocolObservation::Reported(
                OutboundProtocolId::new(engine.clone(), node.protocol).expect("valid protocol"),
            ),
            protocol_display_name: Some(node.protocol.to_owned()),
            cipher: OutboundObservation::Unknown,
            cipher_display_name: None,
            collection_kind: OutboundObservation::Unknown,
            features: OutboundObservation::Observed(if node.tags_udp {
                vec![OutboundFeature::Udp]
            } else {
                Vec::new()
            }),
            endpoint: OutboundObservation::Unknown,
            udp: if node.udp {
                OutboundUdpSupport::Supported
            } else {
                OutboundUdpSupport::Unsupported
            },
            health: if node.healthy {
                OutboundHealthObservation::Healthy
            } else {
                OutboundHealthObservation::Unhealthy
            },
            current_latency_ms: match node.latency {
                Some(delay) => OutboundObservation::Observed(delay),
                None => OutboundObservation::Unknown,
            },
            latency_history: OutboundObservation::Unknown,
            rtt: OutboundObservation::Unknown,
            historical_egress: OutboundObservation::Unknown,
            probe_availability: OutboundProbeAvailability::Supported,
        })
        .collect();
    snapshot.outbound_details = PageData::ready(
        OutboundDetailsSnapshot::new(
            scope(),
            SnapshotRevision::new(10),
            OutboundDetailsState::Ready { details },
        )
        .expect("valid details"),
    );
    let preferences = OutboundUiPreferencesSnapshot::new(
        scope(),
        SnapshotRevision::new(4),
        OutboundUiPreferencesValues {
            search_query: query.to_owned(),
            favorite_outbound_ids: favorites.iter().map(|id| outbound_id(id)).collect(),
            group_order: Vec::new(),
            collapsed_group_ids: Vec::new(),
            sort_order: sort,
            health_filter: OutboundHealthFilter::All,
            layout: OutboundLayout::CompactList,
        },
    )
    .expect("valid engine-scoped preferences");
    snapshot.outbound_ui_preferences = PageData::ready(preferences);
    snapshot
}

/// The owner snapshot the overview page reads.
fn overview_snapshot() -> SurfaceSnapshot {
    let mut snapshot = SurfaceSnapshot::unavailable(
        SurfaceKind::SlintDesktop,
        HostKind::Desktop,
        Failure::new(ErrorCode::NotReady, "initial", true),
    );
    snapshot.pages.settings = PageData::ready(default_settings_snapshot());
    snapshot
}

/// The settings page the owner publishes.
fn default_settings_snapshot() -> SettingsPageSnapshot {
    SettingsPageSnapshot {
        close_to_tray: None,
        notifications_enabled: None,
        language: "zh-CN".into(),
        theme: "dark".into(),
        autostart: false,
        system_proxy: false,
        lan_sharing: None,
        lan_security: None,
        ipv6_routing: None,
        pac: Default::default(),
        tun_enabled: Some(false),
        tun_stack: None,
        tun_auto_route: None,
        tun_strict_route: None,
        controller_port: None,
        log_level: None,
        core_channel: "stable".into(),
        mini_hud: Default::default(),
        refresh_current_profile_on_start: false,
        custom_accent_color: None,
    }
}

/// The node ids the surface projects for the given owner snapshot.
fn projected_ids(surface: &harness::SurfaceHarness) -> Vec<String> {
    surface
        .proxy_node_rows()
        .iter()
        .map(|row| row.id.to_string())
        .collect()
}

#[test]
fn the_proxy_query_answers_the_shared_rule_and_not_a_plain_substring() {
    let surface = harness::SurfaceHarness::new(Some(proxy_snapshot(
        "",
        OutboundSortOrder::LatencyAscending,
        &[],
    )))
    .unwrap();

    // The pinyin initials reach the Chinese node; a plain substring search
    // never finds it.
    surface.publish_snapshot(proxy_snapshot(
        "xg",
        OutboundSortOrder::LatencyAscending,
        &[],
    ));
    assert_eq!(projected_ids(&surface), ["HK-01"]);

    // The `ss` alias reaches the Shadowsocks node. The node's own name, its
    // identity and its protocol spelling carry no `ss`, so the alias is the
    // only path that answers this query.
    surface.publish_snapshot(proxy_snapshot(
        "ss",
        OutboundSortOrder::LatencyAscending,
        &[],
    ));
    assert_eq!(projected_ids(&surface), ["JP-01"]);

    // The `hy2` alias reaches the Hysteria2 node.
    surface.publish_snapshot(proxy_snapshot(
        "hy2",
        OutboundSortOrder::LatencyAscending,
        &[],
    ));
    assert_eq!(projected_ids(&surface), ["JP-02"]);

    // The UDP capability reaches the nodes that carry UDP traffic, whether
    // they advertise the feature tag or not.
    surface.publish_snapshot(proxy_snapshot(
        "udp",
        OutboundSortOrder::LatencyAscending,
        &[],
    ));
    let mut ids = projected_ids(&surface);
    ids.sort();
    assert_eq!(ids, ["HK-01", "JP-01"]);
}

#[test]
fn a_query_that_matches_the_group_keeps_every_member() {
    let surface = harness::SurfaceHarness::new(Some(proxy_snapshot(
        "proxies",
        OutboundSortOrder::LatencyAscending,
        &[],
    )))
    .unwrap();
    surface.publish_snapshot(proxy_snapshot(
        "proxies",
        OutboundSortOrder::LatencyAscending,
        &[],
    ));

    // The group name matches the query, so every member stays visible even
    // though no member carries the query in its own name.
    assert_eq!(projected_ids(&surface).len(), NODES.len());
}

#[test]
fn the_proxy_member_order_is_the_shared_order() {
    let surface = harness::SurfaceHarness::new(Some(proxy_snapshot(
        "",
        OutboundSortOrder::LatencyAscending,
        &[],
    )))
    .unwrap();

    // The unmeasured node always ranks last, in the ascending order and in
    // the descending order alike.
    surface.publish_snapshot(proxy_snapshot("", OutboundSortOrder::LatencyAscending, &[]));
    assert_eq!(
        projected_ids(&surface),
        ["HK-01", "DE-01", "JP-01", "JP-02"]
    );

    surface.publish_snapshot(proxy_snapshot(
        "",
        OutboundSortOrder::LatencyDescending,
        &[],
    ));
    assert_eq!(
        projected_ids(&surface),
        ["JP-01", "DE-01", "HK-01", "JP-02"]
    );

    // A favourite comes first, whatever the selected order is.
    surface.publish_snapshot(proxy_snapshot(
        "",
        OutboundSortOrder::LatencyAscending,
        &["JP-02"],
    ));
    assert_eq!(
        projected_ids(&surface),
        ["JP-02", "HK-01", "DE-01", "JP-01"]
    );
}

#[test]
fn the_widget_sort_vocabulary_is_the_only_spelling_of_a_sort_dimension() {
    let surface = harness::SurfaceHarness::new(Some(proxy_snapshot(
        "",
        OutboundSortOrder::NameDescending,
        &[],
    )))
    .unwrap();

    surface.publish_snapshot(proxy_snapshot("", OutboundSortOrder::NameDescending, &[]));
    assert_eq!(
        surface.window().get_proxy_sort_mode().as_str(),
        widget_vocabulary::proxy_sort_value(OutboundSortOrder::NameDescending)
    );

    surface.publish_snapshot(proxy_snapshot("", OutboundSortOrder::LatencyAscending, &[]));
    assert_eq!(
        surface.window().get_proxy_sort_mode().as_str(),
        widget_vocabulary::proxy_sort_value(OutboundSortOrder::LatencyAscending)
    );
}

#[test]
fn the_traffic_mode_comes_from_the_runtime_readback_and_not_from_the_settings() {
    let mut snapshot = overview_snapshot();
    // The configuration asks for the TUN device, but the runtime never
    // confirmed it, so the surface must not claim the TUN mode.
    let mut settings = default_settings_snapshot();
    settings.tun_enabled = Some(true);
    snapshot.pages.settings = PageData::ready(settings);
    let surface = harness::SurfaceHarness::new(Some(snapshot.clone())).unwrap();
    surface.publish_snapshot(snapshot.clone());

    assert_eq!(surface.network_mode(), "Unknown");
    assert!(
        surface.window().get_network_mode_status_warning(),
        "an unobserved traffic mode must warn"
    );

    // The runtime confirms the TUN device, so the surface reports it.
    snapshot.runtime_control = RuntimeControlSnapshot {
        status: RuntimeControlStatus::Ready,
        tun_enabled: Some(true),
        ..RuntimeControlSnapshot::default()
    };
    snapshot.system_proxy.status = SystemProxyStatus::Disabled;
    surface.publish_snapshot(snapshot);

    assert_eq!(surface.network_mode(), "TUN");
    assert!(!surface.window().get_network_mode_status_warning());
}

#[test]
fn the_traffic_mode_follows_the_shared_system_proxy_readback() {
    let surface = harness::SurfaceHarness::new(Some(overview_snapshot())).unwrap();

    let mut snapshot = overview_snapshot();
    snapshot.system_proxy.status = SystemProxyStatus::Enabled;
    surface.publish_snapshot(snapshot);
    assert_eq!(surface.network_mode(), "System Proxy");
    assert!(!surface.window().get_network_mode_status_warning());

    let mut snapshot = overview_snapshot();
    snapshot.system_proxy.status = SystemProxyStatus::Disabled;
    surface.publish_snapshot(snapshot);
    assert_eq!(surface.network_mode(), "Core only");
}

#[test]
fn the_routing_mode_comes_from_the_shared_routing_intent() {
    let surface = harness::SurfaceHarness::new(Some(overview_snapshot())).unwrap();

    let mut snapshot = overview_snapshot();
    snapshot.core.routing_intent = RoutingIntentSnapshot {
        current: Some(RoutingIntent::RouteAllThroughProxy),
        status: RoutingIntentStatus::Ready,
        failure: None,
    };
    surface.publish_snapshot(snapshot);
    assert_eq!(surface.route_mode(), "Global");

    let mut snapshot = overview_snapshot();
    snapshot.core.routing_intent = RoutingIntentSnapshot {
        current: Some(RoutingIntent::BypassProxy),
        status: RoutingIntentStatus::Ready,
        failure: None,
    };
    surface.publish_snapshot(snapshot);
    assert_eq!(surface.route_mode(), "Direct");
}
