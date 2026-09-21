//! Speedtest & Jitter Benchmark Inspector component.
//!
//! Renders the canonical `SpeedtestSnapshot` published by the shared
//! application engine. There is no UI-local metric: the button dispatches the
//! shared speedtest port and the view only reads typed results.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::components::{BadgeKind, badge, card, style_accent};
use crate::view::svg_icons::{self, Icon};
use crate::view::theme::{self, FONT_MEDIUM, FONT_SEMIBOLD, MONO, tokens};
use iced::widget::{Space, button, column, row, text, text_input};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_contract::speedtest::{
    NodeSpeedtestResult, PacketLossRating, SpeedtestScope, SpeedtestSnapshot,
};
use infiltrator_shared::locales::{Lang, Localizer};

fn loss_badge(rating: PacketLossRating) -> Element<'static, Message> {
    let (label, kind) = match rating {
        PacketLossRating::Excellent => (rating.label(), BadgeKind::Success),
        PacketLossRating::Good => (rating.label(), BadgeKind::Accent),
        PacketLossRating::Fair => (rating.label(), BadgeKind::Warning),
        PacketLossRating::Poor | PacketLossRating::Dead => (rating.label(), BadgeKind::Danger),
    };
    badge(label.to_string(), kind)
}

fn star_label(stars: u8) -> String {
    let filled = stars.min(5) as usize;
    let mut out = String::new();
    for _ in 0..filled {
        out.push('★');
    }
    for _ in filled..5 {
        out.push('☆');
    }
    out
}

fn format_scope(scope: &SpeedtestScope, lang: &Lang<'_>) -> String {
    match scope {
        SpeedtestScope::AllGroups => lang.tr("speedtest_scope_all_groups").to_string(),
        SpeedtestScope::SingleGroup(group) => {
            format!("{} {group}", lang.tr("speedtest_scope_group"))
        }
        SpeedtestScope::SingleNode(node) => {
            format!("{} {node}", lang.tr("speedtest_scope_node"))
        }
    }
}

fn format_run_time(epoch_ms: u64) -> String {
    chrono::DateTime::<chrono::Utc>::from_timestamp_millis(epoch_ms as i64)
        .map(|dt| dt.format("%m-%d %H:%M").to_string())
        .unwrap_or_else(|| "—".to_string())
}

/// Render one compact, honest line per persisted run from the shared snapshot.
///
/// The history is read straight off `snapshot.recent_history`; the view never
/// owns a store or fabricates a run.
pub fn shared_speedtest_history_lines(
    snapshot: &SpeedtestSnapshot,
    lang: &Lang<'_>,
) -> Vec<String> {
    snapshot
        .recent_history
        .iter()
        .rev()
        .map(|record| {
            let bandwidth = record
                .avg_bandwidth_mbps
                .map(|mbps| format!("{mbps:.1} Mbps"))
                .unwrap_or_else(|| "—".to_string());
            let latency = record
                .avg_latency_ms
                .map(|ms| format!("{ms:.1} ms"))
                .unwrap_or_else(|| "—".to_string());
            let jitter = record
                .avg_jitter_ms
                .map(|ms| format!("{ms:.1} ms"))
                .unwrap_or_else(|| "—".to_string());
            let stars = "★".repeat(record.overall_star_rating.min(5) as usize);
            format!(
                "{time} · {scope} · {alive_label} {alive}/{total} · {lat_label} {latency} · {jit_label} {jitter} · {bw_label} {bandwidth} · {stars}",
                time = format_run_time(record.timestamp_epoch_ms),
                scope = format_scope(&record.scope, lang),
                alive_label = lang.tr("speedtest_history_alive"),
                alive = record.alive_nodes,
                total = record.total_nodes,
                lat_label = lang.tr("speedtest_history_latency"),
                latency = latency,
                jit_label = lang.tr("speedtest_history_jitter"),
                jitter = jitter,
                bw_label = lang.tr("speedtest_history_bandwidth"),
                bandwidth = bandwidth,
                stars = stars,
            )
        })
        .collect()
}

pub fn speedtest_card<'a>(state: &'a AppState, lang: &Lang<'_>) -> Element<'a, Message> {
    let snapshot = &state.diag.speedtest;
    let is_running = snapshot.is_running();

    let target = state.runtime.runtime_selected_proxy.clone();

    let run_btn = button(
        row![
            svg_icons::icon_themed(Icon::Zap, 14.0, |t: &Theme| tokens(t).on_accent),
            Space::new().width(theme::SP_SM),
            text(if is_running {
                lang.tr("speedtest_measuring").to_string()
            } else {
                lang.tr("speedtest_btn_start").to_string()
            })
            .size(12)
            .font(FONT_MEDIUM),
        ]
        .align_y(Alignment::Center),
    )
    .padding([6, 14])
    .style(style_accent)
    .on_press_maybe(
        (!is_running && !target.is_empty()).then(|| Message::RunNodeSpeedtest(target.clone())),
    );

    // DUAL-06-03: the typed speedtest target URL. Blank defers to the shared
    // engine default; the typed value rides into `run_scope`/`probe_node`.
    let target_url_input = text_input(
        lang.tr("speedtest_target_url_placeholder").as_ref(),
        &state.runtime.runtime_speedtest_url,
    )
    .on_input(Message::UpdateSpeedtestTestUrl)
    .padding([6, 10])
    .size(12)
    .width(Length::Fill);

    // DUAL-06-01: the live concurrency bound is read from the shared snapshot
    // and written back through the port; the UI never owns the effective fact.
    let concurrency = snapshot.config.concurrency;
    let concurrency_down = button(text("−").size(14))
        .padding([2, 10])
        .on_press(Message::AdjustSpeedtestConcurrency(-1));
    let concurrency_up = button(text("+").size(14))
        .padding([2, 10])
        .on_press(Message::AdjustSpeedtestConcurrency(1));
    let controls_row = row![
        text(lang.tr("speedtest_target_url_label").to_string())
            .size(11)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary)
            }),
        Space::new().width(theme::SP_XS),
        target_url_input,
        Space::new().width(theme::SP_SM),
        text(lang.tr("speedtest_concurrency_label").to_string())
            .size(11)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary)
            }),
        Space::new().width(theme::SP_XS),
        concurrency_down,
        text(concurrency.to_string()).size(13).font(MONO),
        concurrency_up,
    ]
    .align_y(Alignment::Center);

    // Pick the measured row for the active target, if any.
    let result: Option<&NodeSpeedtestResult> = snapshot.node_results.get(&target).or_else(|| {
        // Fall back to the fastest measured node so the card is useful even
        // before the user selects a specific target.
        snapshot.fastest_node()
    });

    let metric_content: Element<'_, Message> = if let Some(res) = result {
        let (bandwidth, jitter_ms, loss) = (
            res.bandwidth_mbps,
            res.jitter.as_ref().map(|j| j.jitter_ms),
            res.packet_loss,
        );

        let bandwidth_text = bandwidth
            .map(|mbps| format!("{mbps:.1} Mbps"))
            .unwrap_or_else(|| "—".to_string());
        let jitter_text = jitter_ms
            .map(|ms| format!("{ms:.1} ms"))
            .unwrap_or_else(|| "—".to_string());
        let loss_text = res
            .jitter
            .as_ref()
            .map(|j| format!("{:.1}%", j.loss_percent))
            .unwrap_or_else(|| "—".to_string());

        row![
            column![
                text(lang.tr("speedtest_bandwidth").to_string())
                    .size(11)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_secondary)
                    }),
                text(bandwidth_text)
                    .size(16)
                    .font(FONT_SEMIBOLD)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).accent)
                    }),
            ]
            .width(Length::Fill),
            column![
                text(lang.tr("speedtest_jitter").to_string())
                    .size(11)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_secondary)
                    }),
                text(jitter_text).size(14).font(MONO),
            ]
            .width(Length::Fill),
            column![
                text(lang.tr("speedtest_packet_loss").to_string())
                    .size(11)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_secondary)
                    }),
                text(loss_text).size(14).font(MONO),
            ]
            .width(Length::Fill),
            column![
                text(lang.tr("speedtest_stability").to_string())
                    .size(11)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_secondary)
                    }),
                row![
                    loss_badge(loss),
                    Space::new().width(theme::SP_XS),
                    text(star_label(res.star_rating)).size(13),
                ]
                .align_y(Alignment::Center),
            ],
        ]
        .align_y(Alignment::Center)
        .into()
    } else {
        row![
            text(format!(
                "Node: {}",
                if target.is_empty() {
                    "No node selected"
                } else {
                    &target
                }
            ))
            .size(12)
            .font(FONT_MEDIUM),
            Space::new().width(Length::Fill),
            text("Click to run bandwidth and packet loss benchmark")
                .size(11)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary)
                }),
        ]
        .align_y(Alignment::Center)
        .into()
    };

    // Timed-out / unreachable nodes are archived in one honest region driven
    // by the shared snapshot's `dead_nodes()` — never hidden or fabricated.
    let dead_nodes = snapshot.dead_nodes();
    let dead_section: Element<'_, Message> = if dead_nodes.is_empty() {
        Space::new().height(0).into()
    } else {
        let names: Vec<String> = dead_nodes
            .iter()
            .take(4)
            .map(|node| node.node_name.clone())
            .collect();
        let extra = dead_nodes.len().saturating_sub(names.len());
        let mut listed = names.join(" · ");
        if extra > 0 {
            listed.push_str(&format!(" (+{extra})"));
        }
        row![
            badge(dead_nodes.len().to_string(), BadgeKind::Danger),
            Space::new().width(theme::SP_XS),
            text(lang.tr("speedtest_dead_archive").to_string())
                .size(11)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary)
                }),
            Space::new().width(theme::SP_XS),
            text(listed)
                .size(11)
                .font(MONO)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).danger)
                }),
        ]
        .align_y(Alignment::Center)
        .into()
    };

    // Persisted history from the shared snapshot: every line is a real run,
    // never a UI-local record.
    let history_lines = shared_speedtest_history_lines(snapshot, lang);
    let mut history_column = column![
        text(lang.tr("speedtest_history_title").to_string())
            .size(11)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary)
            })
    ]
    .spacing(2);
    if history_lines.is_empty() {
        history_column = history_column.push(
            text(lang.tr("speedtest_history_empty").to_string())
                .size(11)
                .font(MONO)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary),
                }),
        );
    } else {
        for line in history_lines {
            history_column = history_column.push(text(line).size(11).font(MONO));
        }
    }
    let history_section: Element<'_, Message> = history_column.into();

    card(
        Some(lang.tr("speedtest_title").to_string()),
        column![
            row![
                text(format!(
                    "Target: {}",
                    if target.is_empty() { "None" } else { &target }
                ))
                .size(12)
                .font(MONO)
                .width(Length::Fill),
                run_btn,
            ]
            .align_y(Alignment::Center),
            Space::new().height(theme::SP_XS),
            controls_row,
            Space::new().height(theme::SP_XS),
            metric_content,
            history_section,
            dead_section,
        ]
        .spacing(theme::SP_SM),
    )
}
