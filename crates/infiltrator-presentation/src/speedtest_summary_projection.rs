//! Shared fact-to-copy projection for typed multi-round speedtest results.

use crate::text::localize;
use chrono::{DateTime, Utc};
use infiltrator_contract::speedtest::{
    PacketLossRating, SpeedtestOutboundResult, SpeedtestSnapshot,
};
use lurker_core_contract::outbound::probe::OutboundProbeBatchScope;
use lurker_ui_presentation::copy::TextCatalog;

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

pub fn rating_label(rating: Option<PacketLossRating>, catalog: &dyn TextCatalog) -> String {
    rating
        .map(|rating| catalog.text(rating_key(rating)).into_owned())
        .unwrap_or_else(|| "—".into())
}

pub fn history_lines(snapshot: &SpeedtestSnapshot, catalog: &dyn TextCatalog) -> Vec<String> {
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
                || catalog.text("speedtest_scope_unattributed").into_owned(),
                |target| match target.outbound_scope() {
                    OutboundProbeBatchScope::All => {
                        catalog.text("speedtest_scope_all_groups").into_owned()
                    }
                    OutboundProbeBatchScope::One(id) => localize(
                        catalog,
                        "speedtest_summary_scope",
                        &[
                            ("kind", catalog.text("speedtest_scope_node").into_owned()),
                            ("name", id.value().to_owned()),
                        ],
                    ),
                    OutboundProbeBatchScope::Collection(id) => localize(
                        catalog,
                        "speedtest_summary_scope",
                        &[
                            ("kind", catalog.text("speedtest_scope_group").into_owned()),
                            ("name", id.value().to_owned()),
                        ],
                    ),
                    OutboundProbeBatchScope::Selected(ids) => localize(
                        catalog,
                        "speedtest_summary_scope",
                        &[
                            (
                                "kind",
                                catalog.text("speedtest_scope_selection").into_owned(),
                            ),
                            ("name", ids.len().to_string()),
                        ],
                    ),
                },
            );
            localize(
                catalog,
                "speedtest_summary_run",
                &[
                    ("time", time),
                    ("scope", scope),
                    (
                        "completed_label",
                        catalog.text("speedtest_history_completed").into_owned(),
                    ),
                    ("completed", record.completed_targets.to_string()),
                    ("total", record.total_targets.to_string()),
                    (
                        "latency_label",
                        catalog.text("speedtest_history_latency").into_owned(),
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
                        catalog.text("speedtest_history_jitter").into_owned(),
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

pub fn dead_archive(snapshot: &SpeedtestSnapshot, catalog: &dyn TextCatalog) -> DeadArchive {
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
            catalog,
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

pub fn metrics_label(node: Option<&SpeedtestOutboundResult>, catalog: &dyn TextCatalog) -> String {
    let Some(node) = node else {
        return "—".into();
    };
    let metrics = node_metrics(node);
    let label = localize(
        catalog,
        "speedtest_summary_metrics",
        &[
            ("jitter", metrics.jitter),
            ("loss", metrics.loss),
            ("rating", rating_label(metrics.rating, catalog)),
            ("stars", metrics.stars),
        ],
    );
    localize(
        catalog,
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

pub fn history_caption(snapshot: &SpeedtestSnapshot, catalog: &dyn TextCatalog) -> String {
    let lines = history_lines(snapshot, catalog);
    if lines.is_empty() {
        "—".into()
    } else {
        localize(
            catalog,
            "speedtest_summary_history",
            &[("history", lines.join(" | "))],
        )
    }
}

/// The breathing phase of the speedtest indicator (EFFECTS-012 #40).
///
/// The phase runs from 0.0 up to 1.0 and back down, so a surface drives a
/// pulsing opacity or a pulsing scale from one number instead of writing an
/// oscillator of its own — and two surfaces breathe in step.
pub fn breathing_phase(elapsed_ms: f32, period_ms: f32) -> f32 {
    if !elapsed_ms.is_finite() || !period_ms.is_finite() || period_ms <= 0.0 {
        return 0.0;
    }
    let position = elapsed_ms.rem_euclid(period_ms) / period_ms;
    let triangle = if position <= 0.5 {
        position * 2.0
    } else {
        (1.0 - position) * 2.0
    };
    triangle.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_breathing_phase_runs_from_zero_to_one_and_back() {
        assert_eq!(breathing_phase(0.0, 1000.0), 0.0);
        assert_eq!(breathing_phase(500.0, 1000.0), 1.0);
        assert_eq!(breathing_phase(1000.0, 1000.0), 0.0);
        assert_eq!(breathing_phase(250.0, 1000.0), 0.5);
        // The phase repeats forever.
        assert_eq!(breathing_phase(1250.0, 1000.0), 0.5);
        for step in 0..100 {
            let phase = breathing_phase(step as f32 * 10.0, 1000.0);
            assert!((0.0..=1.0).contains(&phase));
        }
    }

    #[test]
    fn a_broken_period_holds_the_phase_at_zero() {
        assert_eq!(breathing_phase(100.0, 0.0), 0.0);
        assert_eq!(breathing_phase(100.0, -5.0), 0.0);
        assert_eq!(breathing_phase(f32::NAN, 1000.0), 0.0);
    }
}
