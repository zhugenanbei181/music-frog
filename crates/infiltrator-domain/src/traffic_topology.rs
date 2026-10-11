//! Pure topology derivation from typed runtime observations.

use crate::runtime::Connection;
use infiltrator_contract::outbound::details::{OutboundDetailsState, OutboundObservation};
use infiltrator_contract::outbound::inventory::OutboundObservationSnapshot;
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_contract::traffic_topology::{
    RttJitterHistogram, TrafficTopologyNodeSnapshot, TrafficTopologySnapshot, TrafficTopologyStage,
    TrafficTopologyStatus, adjacent_links,
};

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
    pub connections: &'a [Connection],
    pub outbounds: &'a OutboundObservationSnapshot,
}

/// Build a topology only from facts with stable identity. Connection chains
/// currently expose display names, so active flows produce an explicit
/// Unknown state instead of joining those names to an outbound inventory.
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
        return TrafficTopologySnapshot {
            generation: input.generation,
            revision: input.revision,
            status: TrafficTopologyStatus::Unknown,
            failure: Some(
                "connection chains do not expose typed outbound identities; active route is unavailable"
                    .to_owned(),
            ),
            nodes: Vec::new(),
            links: Vec::new(),
            active_connections,
            flow_bps,
            sniffer_enabled: input.sniffer_enabled,
            jitter_histogram,
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
                entries: vec![OutboundSnapshot {
                    id: outbound_id.clone(),
                    display_name: Some("Node display".to_owned()),
                }],
                collections: vec![OutboundSelectionSet {
                    id: group_id,
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
                details: vec![OutboundDetail {
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
                }],
            },
        )
        .unwrap();
        OutboundObservationSnapshot::new(inventory, details).unwrap()
    }

    #[test]
    fn active_connection_chain_without_typed_identity_is_unknown_not_name_joined() {
        let connections = vec![connection(&["GLOBAL", "香港 01"], "MATCH")];
        let outbounds = outbounds(Vec::new());
        let snapshot = derive(TrafficTopologyInput {
            generation: 4,
            revision: 8,
            lifecycle: CoreLifecycle::Running,
            upload_bps: 1_000.0,
            download_bps: 9_000.0,
            tun_enabled: None,
            tun_stack: None,
            mixed_port: Some(7890),
            http_port: None,
            sniffer_enabled: Some(true),
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
                .is_some_and(|failure| failure.contains("typed outbound identities"))
        );
        assert_eq!(snapshot.flow_bps, 10_000.0);
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
