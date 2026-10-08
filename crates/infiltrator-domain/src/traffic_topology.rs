//! Pure topology derivation from Mihomo runtime observations.

use crate::proxy::Proxy;
use crate::runtime::{ConfigSnapshot, Connection};
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_contract::traffic_topology::{
    RttJitterHistogram, TrafficTopologyNodeSnapshot, TrafficTopologySnapshot, TrafficTopologyStage,
    TrafficTopologyStatus, adjacent_links,
};
use std::collections::{BTreeMap, HashMap};

/// The transport-neutral input to topology derivation.
pub struct TrafficTopologyInput<'a> {
    pub generation: u64,
    pub revision: u64,
    pub lifecycle: CoreLifecycle,
    pub upload_bps: f64,
    pub download_bps: f64,
    pub config: &'a ConfigSnapshot,
    pub connections: &'a [Connection],
    pub proxies: &'a HashMap<String, Proxy>,
}

/// Build the one canonical five-stage routing chain from live controller
/// facts. Mihomo exposes aggregate traffic and connection chains, not a
/// per-packet graph, so links intentionally carry aggregate flow metadata.
pub fn derive(input: TrafficTopologyInput<'_>) -> TrafficTopologySnapshot {
    let active_connections = u32::try_from(input.connections.len()).unwrap_or(u32::MAX);
    let flow_bps = sanitize_rate(input.upload_bps) + sanitize_rate(input.download_bps);
    let has_connections = active_connections > 0;

    let inbound_detail = inbound_detail(input.config);
    let sniffer_enabled = input.config.sniffer.as_ref().map(|sniffer| sniffer.enable);
    let sniffer_detail = match sniffer_enabled {
        Some(true) => "enabled".to_owned(),
        Some(false) => "disabled".to_owned(),
        None => "not reported".to_owned(),
    };

    let rule_detail = summarize_rules(input.connections);
    let group_name = select_group(input.connections, input.proxies);
    let group_detail = group_name
        .as_deref()
        .map(|group| {
            input
                .proxies
                .get(group)
                .map(|proxy| format!("{group} / {}", proxy.proxy_type()))
                .unwrap_or_else(|| group.to_owned())
        })
        .unwrap_or_else(|| "not reported".to_owned());
    let outbound_detail = select_outbound(input.connections, input.proxies, group_name.as_deref());

    let active = has_connections;
    let nodes = vec![
        node(
            TrafficTopologyStage::Inbound,
            "Client / Inbound",
            inbound_detail,
            active_connections,
            active,
        ),
        node(
            TrafficTopologyStage::Sniffer,
            "Sniffer",
            sniffer_detail,
            active_connections,
            active,
        ),
        node(
            TrafficTopologyStage::RuleSet,
            "RuleSet",
            rule_detail,
            active_connections,
            active,
        ),
        node(
            TrafficTopologyStage::ProxyGroup,
            "Proxy Group",
            group_detail,
            active_connections,
            active,
        ),
        node(
            TrafficTopologyStage::Outbound,
            "Outbound Node",
            outbound_detail,
            active_connections,
            active,
        ),
    ];

    TrafficTopologySnapshot {
        generation: input.generation,
        revision: input.revision,
        status: if has_connections {
            TrafficTopologyStatus::Ready
        } else {
            TrafficTopologyStatus::Empty
        },
        failure: None,
        nodes,
        links: adjacent_links(active_connections, flow_bps, active),
        active_connections,
        flow_bps,
        sniffer_enabled,
        jitter_histogram: calculate_jitter_histogram(input.proxies),
    }
}

fn calculate_jitter_histogram(proxies: &HashMap<String, Proxy>) -> Option<RttJitterHistogram> {
    let mut delays = Vec::new();
    for proxy in proxies.values() {
        for entry in proxy.history() {
            if entry.delay > 0 {
                delays.push(entry.delay);
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

fn inbound_detail(config: &ConfigSnapshot) -> String {
    if let Some(tun) = &config.tun
        && tun.enable == Some(true)
    {
        let stack = tun
            .stack
            .as_deref()
            .map(str::trim)
            .filter(|stack| !stack.is_empty())
            .unwrap_or("unknown");
        return format!("TUN · {stack}");
    }
    if config.mixed_port != 0 {
        return format!("Mixed :{}", config.mixed_port);
    }
    if config.port != 0 {
        return format!("HTTP/SOCKS :{}", config.port);
    }
    "not reported".to_owned()
}

fn summarize_rules(connections: &[Connection]) -> String {
    let mut counts = BTreeMap::<String, usize>::new();
    for connection in connections {
        let rule = connection.rule.trim();
        if rule.is_empty() {
            continue;
        }
        *counts.entry(rule.to_owned()).or_default() += 1;
    }
    let total = connections.len();
    let Some((rule, count)) = counts
        .into_iter()
        .max_by(|left, right| left.1.cmp(&right.1).then_with(|| right.0.cmp(&left.0)))
    else {
        return if total == 0 {
            "waiting for rule decision".to_owned()
        } else {
            "rule not reported".to_owned()
        };
    };
    if count == total {
        format!("{rule} · {total} flows")
    } else {
        format!("{rule} · {count}/{total} flows")
    }
}

fn select_group(connections: &[Connection], proxies: &HashMap<String, Proxy>) -> Option<String> {
    let mut counts = BTreeMap::<String, usize>::new();
    for connection in connections {
        for chain in &connection.chains {
            if proxies.get(chain).is_some_and(Proxy::is_group) {
                *counts.entry(chain.clone()).or_default() += 1;
                break;
            }
        }
    }
    if let Some((group, _)) = counts
        .into_iter()
        .max_by(|left, right| left.1.cmp(&right.1).then_with(|| right.0.cmp(&left.0)))
    {
        return Some(group);
    }

    ["GLOBAL", "PROXIES"]
        .iter()
        .find(|name| proxies.get(**name).is_some_and(Proxy::is_group))
        .map(|name| (*name).to_owned())
        .or_else(|| {
            proxies
                .iter()
                .find(|(_, proxy)| proxy.is_group())
                .map(|(name, _)| name.clone())
        })
}

fn select_outbound(
    connections: &[Connection],
    proxies: &HashMap<String, Proxy>,
    group: Option<&str>,
) -> String {
    let mut counts = BTreeMap::<String, usize>::new();
    for connection in connections {
        let mut saw_group = false;
        for chain in &connection.chains {
            if proxies.get(chain).is_some_and(Proxy::is_group) {
                saw_group = group.is_none_or(|wanted| wanted == chain);
                continue;
            }
            if saw_group {
                *counts.entry(chain.clone()).or_default() += 1;
            }
        }
    }
    if let Some((outbound, _)) = counts
        .into_iter()
        .max_by(|left, right| left.1.cmp(&right.1).then_with(|| right.0.cmp(&left.0)))
    {
        return proxy_detail(&outbound, proxies);
    }

    group
        .and_then(|name| proxies.get(name).and_then(Proxy::now))
        .map(|name| proxy_detail(name, proxies))
        .unwrap_or_else(|| "not reported".to_owned())
}

fn proxy_detail(name: &str, proxies: &HashMap<String, Proxy>) -> String {
    let Some(proxy) = proxies.get(name) else {
        return name.to_owned();
    };
    match proxy.delay() {
        Some(delay) => format!("{name} · {} · {delay} ms", proxy.proxy_type()),
        None => format!("{name} · {}", proxy.proxy_type()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proxy::{ProxyBase, ProxyGroup, Shadowsocks};
    #[cfg(test)]
    use crate::runtime::SnifferSnapshot;
    #[cfg(test)]
    use crate::runtime::TunSnapshot;
    use crate::runtime::{ConfigSnapshot, ConnectionMetadata};

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

    fn proxies() -> HashMap<String, Proxy> {
        HashMap::from([
            (
                "GLOBAL".to_owned(),
                Proxy::Selector(ProxyGroup {
                    name: "GLOBAL".to_owned(),
                    now: "香港 01".to_owned(),
                    all: vec!["香港 01".to_owned()],
                    history: Vec::new(),
                }),
            ),
            (
                "香港 01".to_owned(),
                Proxy::Shadowsocks(Shadowsocks {
                    base: ProxyBase {
                        name: "香港 01".to_owned(),
                        delay: Some(38),
                        ..Default::default()
                    },
                    ..Default::default()
                }),
            ),
        ])
    }

    #[test]
    fn derives_five_stage_live_chain_from_connections_and_config() {
        let config = ConfigSnapshot {
            mixed_port: 7890,
            sniffer: Some(SnifferSnapshot { enable: true }),
            ..Default::default()
        };
        let connections = vec![connection(&["GLOBAL", "香港 01"], "MATCH")];
        let proxies = proxies();
        let snapshot = derive(TrafficTopologyInput {
            generation: 4,
            revision: 8,
            lifecycle: CoreLifecycle::Running,
            upload_bps: 1_000.0,
            download_bps: 9_000.0,
            config: &config,
            connections: &connections,
            proxies: &proxies,
        });
        assert_eq!(snapshot.status, TrafficTopologyStatus::Ready);
        assert_eq!(snapshot.nodes.len(), 5);
        assert_eq!(snapshot.links.len(), 4);
        assert_eq!(
            snapshot.node(TrafficTopologyStage::Inbound).unwrap().detail,
            "Mixed :7890"
        );
        assert!(
            snapshot
                .node(TrafficTopologyStage::Outbound)
                .unwrap()
                .detail
                .contains("38 ms")
        );
        assert_eq!(snapshot.flow_bps, 10_000.0);
    }

    #[test]
    fn empty_connections_keep_real_config_but_disable_flow() {
        let config = ConfigSnapshot {
            tun: Some(TunSnapshot {
                enable: Some(true),
                stack: Some("gvisor".to_owned()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let proxies = proxies();
        let snapshot = derive(TrafficTopologyInput {
            generation: 1,
            revision: 2,
            lifecycle: CoreLifecycle::Ready,
            upload_bps: 0.0,
            download_bps: 0.0,
            config: &config,
            connections: &[],
            proxies: &proxies,
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
    fn rule_summary_is_deterministic_and_does_not_fabricate_when_absent() {
        let config = ConfigSnapshot::default();
        let proxies = HashMap::new();
        let connections = vec![connection(&[], "DOMAIN"), connection(&[], "DOMAIN")];
        let snapshot = derive(TrafficTopologyInput {
            generation: 1,
            revision: 1,
            lifecycle: CoreLifecycle::Running,
            upload_bps: 0.0,
            download_bps: 0.0,
            config: &config,
            connections: &connections,
            proxies: &proxies,
        });
        assert_eq!(
            snapshot.node(TrafficTopologyStage::RuleSet).unwrap().detail,
            "DOMAIN · 2 flows"
        );
        assert_eq!(
            snapshot
                .node(TrafficTopologyStage::ProxyGroup)
                .unwrap()
                .detail,
            "not reported"
        );
    }

    #[test]
    fn derives_rtt_jitter_histogram_from_proxy_history() {
        use crate::proxy::ProxyHistory;
        use crate::proxy_observation::RuntimeProxyObservation;

        let mut proxies = HashMap::new();
        let history = vec![
            ProxyHistory {
                time: "2026-10-08T00:00:00Z".into(),
                delay: 45,
            },
            ProxyHistory {
                time: "2026-10-08T00:01:00Z".into(),
                delay: 120,
            },
            ProxyHistory {
                time: "2026-10-08T00:02:00Z".into(),
                delay: 350,
            },
        ];
        proxies.insert(
            "Node-1".into(),
            Proxy::Observed(RuntimeProxyObservation {
                name: "Node-1".into(),
                proxy_type: "ss".into(),
                history: Some(history),
                udp: None,
                alive: Some(true),
                delay: Some(45),
                server: None,
                port: None,
                cipher: None,
                all: None,
                now: None,
            }),
        );

        let config = ConfigSnapshot::default();
        let connections = vec![connection(&["Node-1"], "MATCH")];
        let snapshot = derive(TrafficTopologyInput {
            generation: 1,
            revision: 1,
            lifecycle: CoreLifecycle::Running,
            upload_bps: 0.0,
            download_bps: 0.0,
            config: &config,
            connections: &connections,
            proxies: &proxies,
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
