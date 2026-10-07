//! Speedtest & Jitter Benchmark Inspector component.
//!
//! Renders the canonical `SpeedtestSnapshot` published by the shared
//! application engine. There is no UI-local metric: the button dispatches the
//! shared speedtest port and the view only reads typed results.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::component_card::card;
use crate::view::component_forms::style_accent;
use crate::view::components::{BadgeKind, badge};
use crate::view::svg_icons::Icon;
use crate::view::theme::{FONT_MEDIUM, FONT_SEMIBOLD, MONO, tokens};
use crate::view::{svg_icons, theme};
use iced::widget::{Space, button, column, row, text, text_input};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_application::speedtest_summary_projection::{
    dead_archive, history_lines, node_metrics, rating_label,
};
use infiltrator_contract::speedtest::{EgressCountryMatch, NodeSpeedtestResult, PacketLossRating};
use infiltrator_shared::locales::{Lang, Localizer};

fn loss_badge(rating: Option<PacketLossRating>, lang: &Lang<'_>) -> Element<'static, Message> {
    let kind = match rating {
        Some(PacketLossRating::Excellent) => BadgeKind::Success,
        Some(PacketLossRating::Good) => BadgeKind::Accent,
        Some(PacketLossRating::Fair) => BadgeKind::Warning,
        Some(PacketLossRating::Poor | PacketLossRating::Dead) => BadgeKind::Danger,
        None => BadgeKind::Neutral,
    };
    badge(rating_label(rating, lang.0), kind)
}

/// DUAL-06-12: honest match/mismatch badge for the label-vs-egress country.
fn egress_match_badge(matched: EgressCountryMatch, lang: &Lang<'_>) -> Element<'static, Message> {
    let (key, kind) = match matched {
        EgressCountryMatch::Match => ("speedtest_detail_match", BadgeKind::Success),
        EgressCountryMatch::Mismatch => ("speedtest_detail_mismatch", BadgeKind::Danger),
        EgressCountryMatch::Unlabelled => ("speedtest_detail_unlabelled", BadgeKind::Neutral),
        EgressCountryMatch::Unknown => ("speedtest_detail_unknown", BadgeKind::Neutral),
    };
    badge(lang.tr(key).to_string(), kind)
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
        let metrics = node_metrics(res);
        let bandwidth_text = metrics.bandwidth;
        let jitter_text = metrics.jitter;
        let loss_text = metrics.loss;
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
                    loss_badge(metrics.rating, lang),
                    Space::new().width(theme::SP_XS),
                    text(metrics.stars).size(13),
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
            text(lang.tr("speedtest_benchmark_hint").into_owned())
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
    let archive = dead_archive(snapshot, lang.0);
    let dead_section: Element<'_, Message> = if archive.count == 0 {
        Space::new().height(0).into()
    } else {
        row![
            badge(archive.count.to_string(), BadgeKind::Danger),
            Space::new().width(theme::SP_XS),
            text(lang.tr("speedtest_dead_archive").to_string())
                .size(11)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary)
                }),
            Space::new().width(theme::SP_XS),
            text(archive.names)
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
    let history_lines = history_lines(snapshot, lang.0);
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

    // DUAL-06-12: the reported egress endpoint + honest label-vs-egress match.
    let egress_section: Element<'_, Message> = if let Some(res) = result {
        row![
            text(lang.tr("speedtest_detail_egress").to_string())
                .size(11)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary)
                }),
            Space::new().width(theme::SP_XS),
            text(res.egress_endpoint_label()).size(12).font(MONO),
            Space::new().width(theme::SP_XS),
            egress_match_badge(res.egress_country_match(), lang),
            Space::new().width(Length::Fill),
            text(snapshot.egress_summary())
                .size(10)
                .font(MONO)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary)
                }),
        ]
        .align_y(Alignment::Center)
        .into()
    } else {
        Space::new().height(0).into()
    };

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
                // DUAL-06-13: open the per-node detail modal over the shared
                // snapshot; the modal owns no metrics of its own.
                button(
                    text(lang.tr("speedtest_detail_open").to_string())
                        .size(11)
                        .font(FONT_MEDIUM)
                )
                .padding([6, 12])
                .style(style_accent)
                .on_press(Message::OpenSpeedtestDetail),
                Space::new().width(theme::SP_SM),
                run_btn,
            ]
            .align_y(Alignment::Center),
            Space::new().height(theme::SP_XS),
            controls_row,
            Space::new().height(theme::SP_XS),
            metric_content,
            egress_section,
            history_section,
            dead_section,
        ]
        .spacing(theme::SP_SM),
    )
}
