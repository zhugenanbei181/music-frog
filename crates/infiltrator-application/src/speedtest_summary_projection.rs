//! Shared fact-to-copy projection for typed multi-round speedtest results.

use chrono::{DateTime, Utc};
use infiltrator_contract::outbound::probe::OutboundProbeBatchScope;
use infiltrator_contract::speedtest::{
    PacketLossRating, SpeedtestOutboundResult, SpeedtestSnapshot,
};
use infiltrator_shared::i18n_interpolator::localize;
use infiltrator_shared::locales::{Lang, Localizer};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeMetrics {
    pub jitter: String,
    pub loss: String,
    pub rating: Option<PacketLossRating>,
    pub stars: String,
}

pub fn stars(value: u8) -> String {
    let filled = value.min(5) as usize;
    format!("{}{}", "★".repeat(filled), "☆".repeat(5 - filled))
}

pub fn node_metrics(node: &SpeedtestOutboundResult) -> NodeMetrics {
    let calculation = node.calculation();
    let jitter = calculation
        .as_ref()
        .filter(|value| value.successful_probes >= 2);
    NodeMetrics {
        jitter: jitter
            .map(|value| format!("{:.1} ms", value.jitter_ms))
            .unwrap_or_else(|| "—".into()),
        loss: calculation
            .as_ref()
            .map(|value| format!("{:.1}%", value.loss_percent))
            .unwrap_or_else(|| "—".into()),
        rating: calculation.as_ref().map(|value| value.loss_rating),
        stars: calculation
            .as_ref()
            .map(|value| stars(value.star_rating))
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
            let scope = record.target.as_ref().map_or_else(
                || lang.tr("speedtest_scope_unattributed").into_owned(),
                |target| match target.outbound_scope() {
                    OutboundProbeBatchScope::All => {
                        lang.tr("speedtest_scope_all_groups").into_owned()
                    }
                    OutboundProbeBatchScope::One(id) => localize(
                        locale,
                        "speedtest_summary_scope",
                        &[
                            ("kind", lang.tr("speedtest_scope_node").into_owned()),
                            ("name", id.value().to_owned()),
                        ],
                    ),
                    OutboundProbeBatchScope::Collection(id) => localize(
                        locale,
                        "speedtest_summary_scope",
                        &[
                            ("kind", lang.tr("speedtest_scope_group").into_owned()),
                            ("name", id.value().to_owned()),
                        ],
                    ),
                    OutboundProbeBatchScope::Selected(ids) => localize(
                        locale,
                        "speedtest_summary_scope",
                        &[
                            ("kind", lang.tr("speedtest_scope_selection").into_owned()),
                            ("name", ids.len().to_string()),
                        ],
                    ),
                },
            );
            localize(
                locale,
                "speedtest_summary_run",
                &[
                    ("time", time),
                    ("scope", scope),
                    (
                        "completed_label",
                        lang.tr("speedtest_history_completed").into_owned(),
                    ),
                    ("completed", record.completed_targets.to_string()),
                    ("total", record.total_targets.to_string()),
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
    let nodes = snapshot.timed_out();
    let count = nodes.len();
    let mut names = nodes
        .iter()
        .take(4)
        .map(|node| {
            node.display_name
                .as_deref()
                .unwrap_or_else(|| node.outbound_id.value())
        })
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

pub fn metrics_label(node: Option<&SpeedtestOutboundResult>, locale: &str) -> String {
    let Some(node) = node else {
        return "—".into();
    };
    let metrics = node_metrics(node);
    let label = localize(
        locale,
        "speedtest_summary_metrics",
        &[
            ("jitter", metrics.jitter),
            ("loss", metrics.loss),
            ("rating", rating_label(metrics.rating, locale)),
            ("stars", metrics.stars),
        ],
    );
    localize(
        locale,
        "speedtest_summary_node",
        &[
            (
                "node",
                node.display_name
                    .clone()
                    .unwrap_or_else(|| node.outbound_id.value().to_owned()),
            ),
            ("metrics", label),
        ],
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
