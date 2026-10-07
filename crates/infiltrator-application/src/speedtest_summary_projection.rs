//! One fact-to-copy fold for speedtest metrics, history and egress on both peers.

use crate::speedtest_detail_projection::match_key;
use chrono::{DateTime, Utc};
use infiltrator_contract::speedtest::{
    NodeSpeedtestResult, PacketLossRating, SpeedtestScope, SpeedtestSnapshot,
};
use infiltrator_shared::i18n_interpolator::localize;
use infiltrator_shared::locales::{Lang, Localizer};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeMetrics {
    pub bandwidth: String,
    pub jitter: String,
    pub loss: String,
    pub rating: Option<PacketLossRating>,
    pub stars: String,
}

pub fn stars(value: u8) -> String {
    let filled = value.min(5) as usize;
    format!("{}{}", "★".repeat(filled), "☆".repeat(5 - filled))
}

pub fn node_metrics(node: &NodeSpeedtestResult) -> NodeMetrics {
    let jitter = node.jitter.as_ref().filter(|value| value.sample_count > 0);
    NodeMetrics {
        bandwidth: node
            .bandwidth_mbps
            .map(|value| format!("{value:.1} Mbps"))
            .unwrap_or_else(|| "—".into()),
        jitter: jitter
            .filter(|value| value.successful_probes >= 2)
            .map(|value| format!("{:.1} ms", value.jitter_ms))
            .unwrap_or_else(|| "—".into()),
        loss: jitter
            .map(|value| format!("{:.1}%", value.loss_percent))
            .unwrap_or_else(|| "—".into()),
        // Bandwidth-only and egress-only observations carry legacy placeholder
        // ratings; they do not prove packet loss or stability was measured.
        rating: jitter.map(|value| value.loss_rating),
        stars: jitter
            .map(|_| stars(node.star_rating))
            .unwrap_or_else(|| "—".into()),
    }
}

pub fn rating_key(rating: PacketLossRating) -> &'static str {
    match rating {
        PacketLossRating::Excellent => "speedtest_loss_excellent",
        PacketLossRating::Good => "speedtest_loss_good",
        PacketLossRating::Fair => "speedtest_loss_fair",
        PacketLossRating::Poor => "speedtest_loss_poor",
        PacketLossRating::Dead => "speedtest_loss_dead",
    }
}

pub fn rating_label(rating: Option<PacketLossRating>, locale: &str) -> String {
    rating
        .map(|rating| Lang(locale).tr(rating_key(rating)).into_owned())
        .unwrap_or_else(|| "—".into())
}

pub fn history_lines(snapshot: &SpeedtestSnapshot, locale: &str) -> Vec<String> {
    let lang = Lang(locale);
    snapshot
        .recent_history
        .iter()
        .rev()
        .map(|record| {
            let time = i64::try_from(record.timestamp_epoch_ms)
                .ok()
                .and_then(DateTime::<Utc>::from_timestamp_millis)
                .map(|time| time.format("%m-%d %H:%M").to_string())
                .unwrap_or_else(|| "—".into());
            let scope = match &record.scope {
                SpeedtestScope::AllGroups => lang.tr("speedtest_scope_all_groups").into_owned(),
                SpeedtestScope::SingleGroup(name) | SpeedtestScope::SingleNode(name) => localize(
                    locale,
                    "speedtest_summary_scope",
                    &[
                        (
                            "kind",
                            lang.tr(if matches!(record.scope, SpeedtestScope::SingleGroup(_)) {
                                "speedtest_scope_group"
                            } else {
                                "speedtest_scope_node"
                            })
                            .into_owned(),
                        ),
                        ("name", name.clone()),
                    ],
                ),
            };
            localize(
                locale,
                "speedtest_summary_run",
                &[
                    ("time", time),
                    ("scope", scope),
                    (
                        "alive_label",
                        lang.tr("speedtest_history_alive").into_owned(),
                    ),
                    ("alive", record.alive_nodes.to_string()),
                    ("total", record.total_nodes.to_string()),
                    (
                        "latency_label",
                        lang.tr("speedtest_history_latency").into_owned(),
                    ),
                    (
                        "latency",
                        record
                            .avg_latency_ms
                            .map(|value| format!("{value:.1} ms"))
                            .unwrap_or_else(|| "—".into()),
                    ),
                    (
                        "jitter_label",
                        lang.tr("speedtest_history_jitter").into_owned(),
                    ),
                    (
                        "jitter",
                        record
                            .avg_jitter_ms
                            .map(|value| format!("{value:.1} ms"))
                            .unwrap_or_else(|| "—".into()),
                    ),
                    (
                        "bandwidth_label",
                        lang.tr("speedtest_history_bandwidth").into_owned(),
                    ),
                    (
                        "bandwidth",
                        record
                            .avg_bandwidth_mbps
                            .map(|value| format!("{value:.1} Mbps"))
                            .unwrap_or_else(|| "—".into()),
                    ),
                    ("stars", stars(record.overall_star_rating)),
                ],
            )
        })
        .collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeadArchive {
    pub count: usize,
    pub names: String,
    pub caption: String,
}

pub fn dead_archive(snapshot: &SpeedtestSnapshot, locale: &str) -> DeadArchive {
    let nodes = snapshot.dead_nodes();
    let count = nodes.len();
    let mut names = nodes
        .iter()
        .take(4)
        .map(|node| node.node_name.as_str())
        .collect::<Vec<_>>()
        .join(" · ");
    let extra = count.saturating_sub(4);
    if extra > 0 {
        names.push_str(&format!(" (+{extra})"));
    }
    let caption = if count == 0 {
        "—".into()
    } else {
        localize(
            locale,
            "speedtest_summary_dead",
            &[("count", count.to_string()), ("nodes", names.clone())],
        )
    };
    DeadArchive {
        count,
        names,
        caption,
    }
}

pub fn egress_label(node: Option<&NodeSpeedtestResult>, locale: &str) -> String {
    node.map(|node| {
        localize(
            locale,
            "speedtest_summary_egress",
            &[
                ("endpoint", node.egress_endpoint_label()),
                (
                    "status",
                    Lang(locale)
                        .tr(match_key(node.egress_country_match()))
                        .into_owned(),
                ),
            ],
        )
    })
    .unwrap_or_else(|| "—".into())
}

pub fn metrics_label(node: Option<&NodeSpeedtestResult>, locale: &str) -> String {
    let Some(node) = node else {
        return "—".into();
    };
    let metrics = node_metrics(node);
    let label = localize(
        locale,
        "speedtest_summary_metrics",
        &[
            ("bandwidth", metrics.bandwidth),
            ("jitter", metrics.jitter),
            ("loss", metrics.loss),
            ("rating", rating_label(metrics.rating, locale)),
            ("stars", metrics.stars),
        ],
    );
    localize(
        locale,
        "speedtest_summary_node",
        &[("node", node.node_name.clone()), ("metrics", label)],
    )
}

pub fn history_caption(snapshot: &SpeedtestSnapshot, locale: &str) -> String {
    let lines = history_lines(snapshot, locale);
    if lines.is_empty() {
        "—".into()
    } else {
        localize(
            locale,
            "speedtest_summary_history",
            &[("history", lines.join(" | "))],
        )
    }
}

#[cfg(test)]
#[path = "speedtest_summary_projection_tests.rs"]
mod tests;
