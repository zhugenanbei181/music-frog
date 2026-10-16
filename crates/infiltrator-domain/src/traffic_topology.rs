//! Pure topology derivation from typed runtime observations.

use crate::runtime::Connection;
use infiltrator_contract::core_identity::CoreScope;
use infiltrator_contract::outbound::details::{OutboundDetailsState, OutboundObservation};
use infiltrator_contract::outbound::identity::OutboundId;
use infiltrator_contract::outbound::inventory::{
    OutboundInventoryState, OutboundObservationSnapshot,
};
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_contract::traffic_topology::{
    RttJitterHistogram, TrafficTopologyNodeSnapshot, TrafficTopologySnapshot, TrafficTopologyStage,
    TrafficTopologyStatus, adjacent_links,
};
use std::collections::BTreeSet;

/// The transport-neutral input to topology derivation.
pub struct TrafficTopologyInput<'a> {
    pub generation: u64,
    pub revision: u64,
    pub lifecycle: CoreLifecycle,
    pub upload_bps: f64,
    pub download_bps: f64,
    pub tun_enabled: Option<bool>,
    pub tun_stack: Option<&'a str>,
    pub mixed_port: Option<u16>,
    pub http_port: Option<u16>,
    pub sniffer_enabled: Option<bool>,
    pub connection_scope: &'a CoreScope,
    pub connections: &'a [Connection],
    pub outbounds: &'a OutboundObservationSnapshot,
}

/// Build a topology only from facts with stable identity. Connection chains
/// are resolved against native outbound keys inside one Core scope; display
/// labels are used only after an identity has been established.
pub fn derive(input: TrafficTopologyInput<'_>) -> TrafficTopologySnapshot {
    let active_connections = u32::try_from(input.connections.len()).unwrap_or(u32::MAX);
    let flow_bps = sanitize_rate(input.upload_bps) + sanitize_rate(input.download_bps);
    let has_connections = active_connections > 0;

    let inbound_detail = inbound_detail(
        input.tun_enabled,
        input.tun_stack,
        input.mixed_port,
        input.http_port,
    );
    let sniffer_detail = match input.sniffer_enabled {
        Some(true) => "enabled".to_owned(),
        Some(false) => "disabled".to_owned(),
        None => "not reported".to_owned(),
    };

    let jitter_histogram = calculate_jitter_histogram(input.outbounds);
    if has_connections {
        return match resolve_connection_route(
            input.connection_scope,
            input.connections,
            input.outbounds,
        ) {
            ConnectionRouteResolution::Resolved { path } => active_topology(
                &input,
                &path,
                inbound_detail,
                sniffer_detail,
                active_connections,
                flow_bps,
                jitter_histogram,
            ),
            ConnectionRouteResolution::Unresolved(reason) => TrafficTopologySnapshot {
                generation: input.generation,
                revision: input.revision,
                status: TrafficTopologyStatus::Unknown,
                failure: Some(reason.as_str().to_owned()),
                nodes: Vec::new(),
                links: Vec::new(),
                active_connections,
                flow_bps,
                sniffer_enabled: input.sniffer_enabled,
                jitter_histogram,
            },
        };
    }

    let nodes = vec![
        node(
            TrafficTopologyStage::Inbound,
            "Client / Inbound",
            inbound_detail,
            active_connections,
            false,
        ),
        node(
            TrafficTopologyStage::Sniffer,
            "Sniffer",
            sniffer_detail,
            active_connections,
            false,
        ),
        node(
            TrafficTopologyStage::RuleSet,
            "RuleSet",
            "waiting for rule decision".to_owned(),
            active_connections,
            false,
        ),
        node(
            TrafficTopologyStage::ProxyGroup,
            "Proxy Group",
            "no active connection route".to_owned(),
            active_connections,
            false,
        ),
        node(
            TrafficTopologyStage::Outbound,
            "Outbound Node",
            "no active connection route".to_owned(),
            active_connections,
            false,
        ),
    ];

    TrafficTopologySnapshot {
        generation: input.generation,
        revision: input.revision,
        status: TrafficTopologyStatus::Empty,
        failure: None,
        nodes,
        links: adjacent_links(active_connections, flow_bps, false),
        active_connections,
        flow_bps,
        sniffer_enabled: input.sniffer_enabled,
        jitter_histogram,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ConnectionRouteResolution {
    Resolved { path: Vec<OutboundId> },
    Unresolved(ConnectionRouteFailure),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ConnectionRouteFailure {
    ScopeMismatch,
    InventoryUnavailable,
    EmptyChain,
    NativeKeyUnmatched,
    NativeKeyAmbiguous,
    MultipleDistinctPaths,
    PathEndsAtGroup,
}

impl ConnectionRouteFailure {
    const fn as_str(self) -> &'static str {
        match self {
            Self::ScopeMismatch => "connection and outbound observations use different Core scopes",
            Self::InventoryUnavailable => {
                "outbound inventory is not ready for native route resolution"
            }
            Self::EmptyChain => "an active connection did not report a route chain",
            Self::NativeKeyUnmatched => {
                "a connection route key did not match a native outbound identity"
            }
            Self::NativeKeyAmbiguous => {
                "a connection route key matched multiple native outbound identities"
            }
            Self::MultipleDistinctPaths => {
                "active connections report multiple distinct outbound paths"
            }
            Self::PathEndsAtGroup => {
                "a connection route ends at a proxy group without a leaf outbound"
            }
        }
    }
}

fn resolve_connection_route(
    scope: &CoreScope,
    connections: &[Connection],
    observation: &OutboundObservationSnapshot,
) -> ConnectionRouteResolution {
    if observation.inventory().scope() != scope {
        return ConnectionRouteResolution::Unresolved(ConnectionRouteFailure::ScopeMismatch);
    }
    let OutboundInventoryState::Ready {
        entries,
        collections,
    } = observation.inventory().inventory()
    else {
        return ConnectionRouteResolution::Unresolved(ConnectionRouteFailure::InventoryUnavailable);
    };

    let mut distinct_paths: Vec<Vec<OutboundId>> = Vec::new();
    for connection in connections {
        if connection.chains.is_empty() {
            return ConnectionRouteResolution::Unresolved(ConnectionRouteFailure::EmptyChain);
        }
        let mut path = Vec::with_capacity(connection.chains.len());
        for native_key in &connection.chains {
            let mut matches = entries.iter().filter_map(|entry| {
                (entry.id.engine_id() == &scope.identity.engine && entry.id.value() == native_key)
                    .then_some(entry.id.clone())
            });
            let Some(id) = matches.next() else {
                return ConnectionRouteResolution::Unresolved(
                    ConnectionRouteFailure::NativeKeyUnmatched,
                );
            };
            if matches.next().is_some() {
                return ConnectionRouteResolution::Unresolved(
                    ConnectionRouteFailure::NativeKeyAmbiguous,
                );
            }
            path.push(id);
        }
        let last = path.last().expect("non-empty connection chain");
        if collections.iter().any(|collection| &collection.id == last) {
            return ConnectionRouteResolution::Unresolved(ConnectionRouteFailure::PathEndsAtGroup);
        }
        if distinct_paths.iter().all(|known| known != &path) {
            distinct_paths.push(path);
        }
    }

    match distinct_paths.as_slice() {
        [path] => ConnectionRouteResolution::Resolved { path: path.clone() },
        [] => ConnectionRouteResolution::Unresolved(ConnectionRouteFailure::EmptyChain),
        _ => ConnectionRouteResolution::Unresolved(ConnectionRouteFailure::MultipleDistinctPaths),
    }
}

fn active_topology(
    input: &TrafficTopologyInput<'_>,
    path: &[OutboundId],
    inbound_detail: String,
    sniffer_detail: String,
    active_connections: u32,
    flow_bps: f64,
    jitter_histogram: Option<RttJitterHistogram>,
) -> TrafficTopologySnapshot {
    let OutboundInventoryState::Ready {
        entries,
        collections,
    } = input.outbounds.inventory().inventory()
    else {
        unreachable!("route resolution requires a ready inventory")
    };
    let display = |id: &OutboundId| {
        entries
            .iter()
            .find(|entry| &entry.id == id)
            .and_then(|entry| entry.display_name.as_deref())
            .unwrap_or_else(|| id.value())
            .to_owned()
    };
    let outbound = path.last().expect("resolved path has a leaf outbound");
    let group = path
        .iter()
        .take(path.len().saturating_sub(1))
        .rev()
        .find(|id| collections.iter().any(|collection| &collection.id == *id));
    let rule_details = input
        .connections
        .iter()
        .map(|connection| connection.rule.trim())
        .filter(|rule| !rule.is_empty())
        .collect::<BTreeSet<_>>();
    let rule_detail = match rule_details.iter().copied().collect::<Vec<_>>().as_slice() {
        [] => "rule not reported".to_owned(),
        [rule] => (*rule).to_owned(),
        _ => "multiple reported rules".to_owned(),
    };
    let nodes = vec![
        node(
            TrafficTopologyStage::Inbound,
            "Client / Inbound",
            inbound_detail,
            active_connections,
            true,
        ),
        node(
            TrafficTopologyStage::Sniffer,
            "Sniffer",
            sniffer_detail,
            active_connections,
            input.sniffer_enabled == Some(true),
        ),
        node(
            TrafficTopologyStage::RuleSet,
            "RuleSet",
            rule_detail,
            active_connections,
            true,
        ),
        node(
            TrafficTopologyStage::ProxyGroup,
            "Proxy Group",
            group
                .map(display)
                .unwrap_or_else(|| "no group reported".to_owned()),
            active_connections,
            group.is_some(),
        ),
        node(
            TrafficTopologyStage::Outbound,
            "Outbound Node",
            display(outbound),
            active_connections,
            true,
        ),
    ];
    TrafficTopologySnapshot {
        generation: input.generation,
        revision: input.revision,
        status: TrafficTopologyStatus::Ready,
        failure: None,
        nodes,
        links: adjacent_links(active_connections, flow_bps, true),
        active_connections,
        flow_bps,
        sniffer_enabled: input.sniffer_enabled,
        jitter_histogram,
    }
}

fn calculate_jitter_histogram(
    observation: &OutboundObservationSnapshot,
) -> Option<RttJitterHistogram> {
    let mut delays = Vec::new();
    if let OutboundDetailsState::Ready { details } = observation.details().state() {
        for detail in details {
            if let OutboundObservation::Observed(history) = &detail.latency_history {
                for entry in history {
                    if entry.delay_ms > 0 {
                        delays.push(entry.delay_ms);
                    }
                }
            }
        }
    }
    if delays.is_empty() {
        return None;
    }
    delays.sort_unstable();

    let mut hist = RttJitterHistogram::default();
    for &d in &delays {
        match d {
            0..=50 => hist.bucket_0_50_ms += 1,
            51..=100 => hist.bucket_50_100_ms += 1,
            101..=200 => hist.bucket_100_200_ms += 1,
            201..=500 => hist.bucket_200_500_ms += 1,
            _ => hist.bucket_over_500_ms += 1,
        }
    }

    let p95_idx = ((delays.len() as f64) * 0.95).ceil() as usize;
    hist.jitter_p95_ms = delays.get(p95_idx.saturating_sub(1)).copied().unwrap_or(0);

    Some(hist)
}

fn node(
    stage: TrafficTopologyStage,
    label: &str,
    detail: String,
    active_connections: u32,
    active: bool,
) -> TrafficTopologyNodeSnapshot {
    TrafficTopologyNodeSnapshot {
        stage,
        label: label.to_owned(),
        detail,
        active_connections,
        active,
    }
}

fn sanitize_rate(rate: f64) -> f64 {
    if rate.is_finite() && rate >= 0.0 {
        rate
    } else {
        0.0
    }
}

fn inbound_detail(
    tun_enabled: Option<bool>,
    tun_stack: Option<&str>,
    mixed_port: Option<u16>,
    http_port: Option<u16>,
) -> String {
    if tun_enabled == Some(true) {
        let stack = tun_stack
            .map(str::trim)
            .filter(|stack| !stack.is_empty())
            .unwrap_or("unknown");
        return format!("TUN · {stack}");
    }
    if let Some(port) = mixed_port.filter(|port| *port != 0) {
        return format!("Mixed :{port}");
    }
    if let Some(port) = http_port.filter(|port| *port != 0) {
        return format!("HTTP/SOCKS :{port}");
    }
    "not reported".to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::ConnectionMetadata;
    use infiltrator_contract::core_identity::{
        CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, EngineId,
    };
    use infiltrator_contract::outbound::details::{
        OutboundDetail, OutboundDetailsSnapshot, OutboundDetailsState, OutboundHealthObservation,
        OutboundLatencySample, OutboundObservation, OutboundProbeAvailability,
        OutboundProtocolObservation, OutboundSnapshot, OutboundUdpSupport,
    };
    use infiltrator_contract::outbound::identity::{OutboundId, SnapshotRevision};
    use infiltrator_contract::outbound::inventory::{
        OutboundInventorySnapshot, OutboundInventoryState, OutboundObservationSnapshot,
        OutboundSelectionSet, OutboundSelectionState, SelectionAvailability,
    };

    fn connection(chains: &[&str], rule: &str) -> Connection {
        Connection {
            id: chains.join("/"),
            metadata: ConnectionMetadata::default(),
            upload: 0,
            download: 0,
            start: String::new(),
            rule: rule.to_owned(),
            rule_payload: String::new(),
            chains: chains.iter().map(|value| (*value).to_owned()).collect(),
        }
    }

    fn outbounds(history: Vec<OutboundLatencySample>) -> OutboundObservationSnapshot {
        let engine = EngineId::new("mihomo").unwrap();
        let scope = CoreScope {
            identity: CoreIdentity::new(
                engine.clone(),
                CoreInstanceId::new("active").unwrap(),
                None,
            ),
            generation: CoreGeneration::new(1),
            session_token: None,
        };
        let group_id = OutboundId::new(engine.clone(), "group-1").unwrap();
        let outbound_id = OutboundId::new(engine.clone(), "out-1").unwrap();
        let inventory = OutboundInventorySnapshot::new(
            scope.clone(),
            SnapshotRevision::new(7),
            OutboundInventoryState::Ready {
                entries: vec![
                    OutboundSnapshot {
                        id: group_id.clone(),
                        display_name: Some("Group display".to_owned()),
                    },
                    OutboundSnapshot {
                        id: outbound_id.clone(),
                        display_name: Some("Node display".to_owned()),
                    },
                ],
                collections: vec![OutboundSelectionSet {
                    id: group_id.clone(),
                    member_ids: vec![outbound_id.clone()],
                    availability: SelectionAvailability::Selectable,
                    selection: OutboundSelectionState::Ready {
                        selected: outbound_id.clone(),
                    },
                }],
            },
        )
        .unwrap();
        let details = OutboundDetailsSnapshot::new(
            scope,
            SnapshotRevision::new(8),
            OutboundDetailsState::Ready {
                details: vec![
                    OutboundDetail {
                        id: group_id,
                        protocol: OutboundProtocolObservation::Unknown,
                        protocol_display_name: None,
                        cipher: OutboundObservation::Unknown,
                        cipher_display_name: None,
                        collection_kind: OutboundObservation::Unknown,
                        features: OutboundObservation::Unknown,
                        endpoint: OutboundObservation::Unknown,
                        udp: OutboundUdpSupport::Unknown,
                        health: OutboundHealthObservation::Unknown,
                        current_latency_ms: OutboundObservation::Unknown,
                        latency_history: OutboundObservation::Unknown,
                        rtt: OutboundObservation::Unknown,
                        historical_egress: OutboundObservation::Unsupported,
                        probe_availability: OutboundProbeAvailability::Unknown,
                    },
                    OutboundDetail {
                        id: outbound_id,
                        protocol: OutboundProtocolObservation::Unknown,
                        protocol_display_name: None,
                        cipher: OutboundObservation::Unknown,
                        cipher_display_name: None,
                        collection_kind: OutboundObservation::Unknown,
                        features: OutboundObservation::Unknown,
                        endpoint: OutboundObservation::Unknown,
                        udp: OutboundUdpSupport::Unknown,
                        health: OutboundHealthObservation::Unknown,
                        current_latency_ms: OutboundObservation::Unknown,
                        latency_history: OutboundObservation::Observed(history),
                        rtt: OutboundObservation::Unknown,
                        historical_egress: OutboundObservation::Unsupported,
                        probe_availability: OutboundProbeAvailability::Unknown,
                    },
                ],
            },
        )
        .unwrap();
        OutboundObservationSnapshot::new(inventory, details).unwrap()
    }

    #[test]
    fn unresolved_native_chain_is_unknown_not_name_joined() {
        let connections = vec![connection(&["GLOBAL", "香港 01"], "MATCH")];
        let outbounds = outbounds(Vec::new());
        let snapshot = derive(TrafficTopologyInput {
            generation: 1,
            revision: 8,
            lifecycle: CoreLifecycle::Running,
            upload_bps: 1_000.0,
            download_bps: 9_000.0,
            tun_enabled: None,
            tun_stack: None,
            mixed_port: Some(7890),
            http_port: None,
            sniffer_enabled: Some(true),
            connection_scope: outbounds.inventory().scope(),
            connections: &connections,
            outbounds: &outbounds,
        });
        assert_eq!(snapshot.status, TrafficTopologyStatus::Unknown);
        assert!(snapshot.nodes.is_empty());
        assert!(snapshot.links.is_empty());
        assert_eq!(snapshot.active_connections, 1);
        assert!(
            snapshot
                .failure
                .as_deref()
                .is_some_and(|failure| failure.contains("native outbound identity"))
        );
        assert_eq!(snapshot.flow_bps, 10_000.0);
    }

    #[test]
    fn native_route_keys_resolve_to_typed_ids_before_display_labels_are_used() {
        let connections = vec![connection(&["group-1", "out-1"], "MATCH")];
        let outbounds = outbounds(Vec::new());
        let snapshot = derive(TrafficTopologyInput {
            generation: 1,
            revision: 8,
            lifecycle: CoreLifecycle::Running,
            upload_bps: 250.0,
            download_bps: 750.0,
            tun_enabled: None,
            tun_stack: None,
            mixed_port: Some(7890),
            http_port: None,
            sniffer_enabled: Some(true),
            connection_scope: outbounds.inventory().scope(),
            connections: &connections,
            outbounds: &outbounds,
        });

        assert_eq!(snapshot.status, TrafficTopologyStatus::Ready);
        assert_eq!(
            snapshot
                .node(TrafficTopologyStage::ProxyGroup)
                .unwrap()
                .detail,
            "Group display"
        );
        assert_eq!(
            snapshot
                .node(TrafficTopologyStage::Outbound)
                .unwrap()
                .detail,
            "Node display"
        );
        assert!(snapshot.links.iter().all(|link| link.active));
    }

    #[test]
    fn display_name_is_never_used_as_a_native_route_key() {
        let connections = vec![connection(&["Node display"], "MATCH")];
        let outbounds = outbounds(Vec::new());
        let snapshot = derive(TrafficTopologyInput {
            generation: 1,
            revision: 8,
            lifecycle: CoreLifecycle::Running,
            upload_bps: 0.0,
            download_bps: 0.0,
            tun_enabled: None,
            tun_stack: None,
            mixed_port: None,
            http_port: None,
            sniffer_enabled: None,
            connection_scope: outbounds.inventory().scope(),
            connections: &connections,
            outbounds: &outbounds,
        });

        assert_eq!(snapshot.status, TrafficTopologyStatus::Unknown);
        assert!(
            snapshot
                .failure
                .as_deref()
                .is_some_and(|reason| reason.contains("did not match"))
        );
    }

    #[test]
    fn different_active_routes_remain_unresolved_in_single_path_topology() {
        let connections = vec![
            connection(&["out-1"], "MATCH"),
            connection(&["group-1", "out-1"], "MATCH"),
        ];
        let outbounds = outbounds(Vec::new());
        let snapshot = derive(TrafficTopologyInput {
            generation: 1,
            revision: 8,
            lifecycle: CoreLifecycle::Running,
            upload_bps: 0.0,
            download_bps: 0.0,
            tun_enabled: None,
            tun_stack: None,
            mixed_port: None,
            http_port: None,
            sniffer_enabled: None,
            connection_scope: outbounds.inventory().scope(),
            connections: &connections,
            outbounds: &outbounds,
        });

        assert_eq!(snapshot.status, TrafficTopologyStatus::Unknown);
        assert!(
            snapshot
                .failure
                .as_deref()
                .is_some_and(|reason| reason.contains("multiple distinct"))
        );
    }

    #[test]
    fn empty_connections_keep_real_config_but_disable_flow() {
        let outbounds = outbounds(Vec::new());
        let snapshot = derive(TrafficTopologyInput {
            generation: 1,
            revision: 2,
            lifecycle: CoreLifecycle::Ready,
            upload_bps: 0.0,
            download_bps: 0.0,
            tun_enabled: Some(true),
            tun_stack: Some("gvisor"),
            mixed_port: None,
            http_port: None,
            sniffer_enabled: None,
            connection_scope: outbounds.inventory().scope(),
            connections: &[],
            outbounds: &outbounds,
        });
        assert_eq!(snapshot.status, TrafficTopologyStatus::Empty);
        assert!(snapshot.is_drawable());
        assert!(!snapshot.is_flowing());
        assert_eq!(
            snapshot.node(TrafficTopologyStage::Inbound).unwrap().detail,
            "TUN · gvisor"
        );
    }

    #[test]
    fn active_connection_without_chain_identity_keeps_topology_unavailable() {
        let outbounds = outbounds(Vec::new());
        let connections = vec![connection(&[], "DOMAIN"), connection(&[], "DOMAIN")];
        let snapshot = derive(TrafficTopologyInput {
            generation: 1,
            revision: 1,
            lifecycle: CoreLifecycle::Running,
            upload_bps: 0.0,
            download_bps: 0.0,
            tun_enabled: None,
            tun_stack: None,
            mixed_port: None,
            http_port: None,
            sniffer_enabled: None,
            connection_scope: outbounds.inventory().scope(),
            connections: &connections,
            outbounds: &outbounds,
        });
        assert_eq!(snapshot.status, TrafficTopologyStatus::Unknown);
        assert!(snapshot.nodes.is_empty());
        assert_eq!(snapshot.active_connections, 2);
    }

    #[test]
    fn derives_rtt_jitter_histogram_from_typed_outbound_history() {
        let history = vec![
            OutboundLatencySample {
                observed_at: "2026-10-08T00:00:00Z".into(),
                delay_ms: 45,
            },
            OutboundLatencySample {
                observed_at: "2026-10-08T00:01:00Z".into(),
                delay_ms: 120,
            },
            OutboundLatencySample {
                observed_at: "2026-10-08T00:02:00Z".into(),
                delay_ms: 350,
            },
        ];
        let outbounds = outbounds(history);
        let snapshot = derive(TrafficTopologyInput {
            generation: 1,
            revision: 1,
            lifecycle: CoreLifecycle::Running,
            upload_bps: 0.0,
            download_bps: 0.0,
            tun_enabled: None,
            tun_stack: None,
            mixed_port: None,
            http_port: None,
            sniffer_enabled: None,
            connection_scope: outbounds.inventory().scope(),
            connections: &[],
            outbounds: &outbounds,
        });

        let hist = snapshot.jitter_histogram.expect("jitter histogram");
        assert_eq!(hist.bucket_0_50_ms, 1);
        assert_eq!(hist.bucket_50_100_ms, 0);
        assert_eq!(hist.bucket_100_200_ms, 1);
        assert_eq!(hist.bucket_200_500_ms, 1);
        assert_eq!(hist.bucket_over_500_ms, 0);
        assert_eq!(hist.jitter_p95_ms, 350);
    }
}
