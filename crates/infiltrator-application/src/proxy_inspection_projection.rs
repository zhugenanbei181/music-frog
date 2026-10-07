//! One truthful detail fold for both peer products; delay totals never imply staged timings.
use chrono::DateTime;
use infiltrator_contract::capability::Availability;
use infiltrator_contract::proxies::ProxyGroupClassification;
use infiltrator_contract::proxy_inspection::{
    ProxyDetailField, ProxyInspectionSnapshot, ProxyLatencySample, ProxyRttStatistics,
};
use infiltrator_contract::speedtest::SpeedtestSnapshot;
use infiltrator_contract::surface_snapshot::{ProxiesPageSnapshot, ProxyNodeSnapshot};
use infiltrator_domain::proxy::Proxy;
use std::collections::HashMap;
use std::net::IpAddr;

pub const INSPECTION_HISTORY_LIMIT: usize = 64;

fn timing() -> Availability {
    Availability::Unsupported {
        reason: "The kernel does not report per-proxy DNS/TCP/TLS/TTFB timings".into(),
    }
}
fn nonempty(value: &str) -> Option<String> {
    (!value.is_empty()).then(|| value.to_owned())
}

pub fn project_proxy_inspection(name: &str, proxy: &Proxy) -> ProxyInspectionSnapshot {
    let (server, port, cipher) = match proxy {
        Proxy::Observed(p) => (
            p.server.clone().filter(|value| !value.is_empty()),
            p.port,
            p.cipher.clone().filter(|value| !value.is_empty()),
        ),
        Proxy::Shadowsocks(p) => (nonempty(&p.server), Some(p.port), nonempty(&p.cipher)),
        Proxy::Vmess(p) => (nonempty(&p.server), Some(p.port), nonempty(&p.cipher)),
        Proxy::Trojan(p) => (nonempty(&p.server), Some(p.port), None),
        Proxy::Hysteria2(p) => (nonempty(&p.server), Some(p.port), None),
        Proxy::WireGuard(p) => (
            nonempty(&p.server).or_else(|| nonempty(&p.ip)),
            Some(p.port),
            None,
        ),
        Proxy::Tuic(p) => (nonempty(&p.server), Some(p.port), None),
        Proxy::Vless(p) => (nonempty(&p.server), Some(p.port), None),
        Proxy::Http(p) => (nonempty(&p.server), Some(p.port), None),
        Proxy::Socks5(p) => (nonempty(&p.server), Some(p.port), None),
        Proxy::Snell(p) => (nonempty(&p.server), Some(p.port), None),
        _ => (None, None, None),
    };
    let observed_leaf = !proxy.is_group() && !matches!(proxy, Proxy::Unknown);
    let history = proxy.history();
    ProxyInspectionSnapshot {
        name: name.into(),
        node_type: proxy.proxy_type().into(),
        server,
        port: port.filter(|port| *port != 0),
        cipher,
        udp: proxy.udp_observation(),
        alive: proxy.health_observation(),
        delay_ms: proxy.delay(),
        history: history
            .iter()
            .skip(history.len().saturating_sub(INSPECTION_HISTORY_LIMIT))
            .map(|sample| ProxyLatencySample {
                time: sample.time.clone(),
                delay_ms: sample.delay,
            })
            .collect(),
        history_total: history.len(),
        rtt: ProxyRttStatistics::from_history(
            &history
                .iter()
                .map(|sample| sample.delay)
                .collect::<Vec<_>>(),
        ),
        egress_ip: None,
        egress_country: None,
        egress_recorded_at_epoch_ms: None,
        timing: timing(),
        can_probe: observed_leaf,
    }
}

pub fn inspection_catalogue(proxies: &HashMap<String, Proxy>) -> Vec<ProxyInspectionSnapshot> {
    let mut details: Vec<_> = proxies
        .iter()
        .map(|(name, proxy)| project_proxy_inspection(name, proxy))
        .collect();
    for member in proxies.values().filter_map(Proxy::all).flatten() {
        if !proxies.contains_key(member) && !details.iter().any(|detail| &detail.name == member) {
            details.push(project_proxy_inspection(member, &Proxy::Unknown));
        }
    }
    details.sort_by(|left, right| left.name.cmp(&right.name));
    details
}

/// Sparse snapshots preserve known observations and leave unavailable metadata/history unknown.
pub fn inspect_sparse_node(node: &ProxyNodeSnapshot) -> ProxyInspectionSnapshot {
    ProxyInspectionSnapshot {
        name: node.name.clone(),
        node_type: node.node_type.clone(),
        server: None,
        port: None,
        cipher: None,
        udp: None,
        alive: node.alive,
        delay_ms: node.delay_ms,
        history: vec![],
        history_total: 0,
        rtt: Default::default(),
        egress_ip: None,
        egress_country: None,
        egress_recorded_at_epoch_ms: None,
        timing: timing(),
        can_probe: node.node_type != "Unknown"
            && ProxyGroupClassification::from_str_loose(&node.node_type).is_none(),
    }
}

pub fn with_egress_record(detail: &mut ProxyInspectionSnapshot, speedtest: &SpeedtestSnapshot) {
    detail.egress_ip = None;
    detail.egress_country = None;
    detail.egress_recorded_at_epoch_ms = None;
    if let Some(record) = speedtest
        .node_results
        .values()
        .filter(|record| {
            record.node_name == detail.name
                && record.proxy_type.eq_ignore_ascii_case(&detail.node_type)
                && record
                    .outbound_ip
                    .as_deref()
                    .is_some_and(|ip| ip.parse::<IpAddr>().is_ok())
        })
        .max_by_key(|record| record.tested_at_epoch_ms)
    {
        detail.egress_ip = record.outbound_ip.clone();
        detail.egress_country = record.outbound_country.clone();
        detail.egress_recorded_at_epoch_ms = Some(record.tested_at_epoch_ms);
    }
}

pub fn lookup_inspection(
    page: &ProxiesPageSnapshot,
    name: &str,
) -> Option<ProxyInspectionSnapshot> {
    page.node_details
        .iter()
        .find(|detail| detail.name == name)
        .cloned()
        .or_else(|| {
            page.groups
                .iter()
                .flat_map(|group| &group.proxies)
                .find(|node| node.name == name)
                .map(inspect_sparse_node)
        })
}

/// History zero has no per-sample outcome: leave a gap rather than inventing a success or timeout.
pub struct ProxyHistoryPlot {
    pub samples: Vec<f32>,
    /// Both native renderers use the same zero-based observed range.
    pub ceiling_ms: f32,
}
pub fn history_plot(detail: &ProxyInspectionSnapshot) -> ProxyHistoryPlot {
    let samples: Vec<_> = detail
        .history
        .iter()
        .map(|sample| {
            if sample.delay_ms == 0 {
                f32::NAN
            } else {
                sample.delay_ms as f32
            }
        })
        .collect();
    let ceiling_ms = samples
        .iter()
        .copied()
        .filter(|value| value.is_finite())
        .fold(1.0, f32::max);
    ProxyHistoryPlot {
        samples,
        ceiling_ms,
    }
}

pub fn field_value(
    detail: &ProxyInspectionSnapshot,
    field: ProxyDetailField,
    tr: &impl Fn(&str) -> String,
) -> String {
    let unknown = || tr("proxy_inspection_unknown");
    let flag = |value: Option<bool>, yes: &str, no: &str| {
        value.map_or_else(unknown, |value| tr(if value { yes } else { no }))
    };
    let millis = |value: Option<u32>| {
        value
            .map(|value| format!("{value} ms"))
            .unwrap_or_else(unknown)
    };
    match field {
        ProxyDetailField::Protocol => detail.node_type.clone(),
        ProxyDetailField::Server => detail.server.clone().unwrap_or_else(unknown),
        ProxyDetailField::Port => detail
            .port
            .map(|port| port.to_string())
            .unwrap_or_else(unknown),
        ProxyDetailField::Cipher => detail.cipher.clone().unwrap_or_else(unknown),
        ProxyDetailField::Udp => flag(
            detail.udp,
            "proxy_inspection_supported",
            "proxy_inspection_disabled",
        ),
        ProxyDetailField::Health => flag(
            detail.alive,
            "proxy_inspection_alive",
            "proxy_inspection_dead",
        ),
        ProxyDetailField::Delay => match detail.delay_ms {
            Some(0) => tr("proxy_inspection_unconfirmed_zero"),
            Some(ms) => format!("{ms} ms"),
            None => tr("proxy_inspection_unmeasured"),
        },
        ProxyDetailField::RttMin => millis(detail.rtt.min_ms),
        ProxyDetailField::RttMax => millis(detail.rtt.max_ms),
        ProxyDetailField::RttAvg => millis(detail.rtt.avg_ms),
        ProxyDetailField::Egress => match (&detail.egress_ip, &detail.egress_country) {
            (Some(ip), Some(country)) => format!("{ip} ({country})"),
            (Some(ip), None) => ip.clone(),
            _ => unknown(),
        },
        ProxyDetailField::EgressTime => detail
            .egress_recorded_at_epoch_ms
            .and_then(|value| i64::try_from(value).ok())
            .and_then(DateTime::from_timestamp_millis)
            .map(|value| value.to_rfc3339())
            .unwrap_or_else(unknown),
    }
}

pub fn history_listing(detail: &ProxyInspectionSnapshot, tr: &impl Fn(&str) -> String) -> String {
    if detail.history.is_empty() {
        return tr("proxy_inspection_history_empty");
    }
    let mut lines = vec![format!(
        "{}: {}/{}",
        tr("proxy_inspection_history"),
        detail.history.len(),
        detail.history_total
    )];
    for sample in &detail.history {
        let time = if sample.time.is_empty() {
            tr("proxy_inspection_unknown")
        } else {
            sample.time.clone()
        };
        let delay = if sample.delay_ms == 0 {
            tr("proxy_inspection_unconfirmed_zero")
        } else {
            format!("{} ms", sample.delay_ms)
        };
        lines.push(format!("{time} · {delay}"));
    }
    if detail.history.iter().any(|sample| sample.delay_ms == 0) {
        lines.push(tr("proxy_inspection_zero_reason"));
    }
    lines.join("\n")
}

#[cfg(test)]
#[path = "proxy_inspection_projection_tests.rs"]
mod tests;
