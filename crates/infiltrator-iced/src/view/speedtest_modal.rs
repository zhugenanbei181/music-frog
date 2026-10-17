//! Speedtest & Jitter Benchmark Inspector component.
//!
//! Renders the canonical `SpeedtestSnapshot` published by the shared
//! Core owner. There is no UI-local metric: the button submits a typed Core
//! command and the view only reads the owner snapshot.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::component_card::card;
use crate::view::component_forms::style_accent;
use crate::view::components::{BadgeKind, badge};
use crate::view::svg_icons::Icon;
use crate::view::theme::{FONT_MEDIUM, FONT_SEMIBOLD, MONO, tokens};
use crate::view::{svg_icons, theme};
use iced::widget::{Space, button, column, row, text};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_contract::speedtest::{
    PacketLossRating, SpeedtestOutboundResult, SpeedtestRounds, SpeedtestTarget,
};
use infiltrator_presentation::speedtest_summary_projection::{
    dead_archive, history_lines, node_metrics, rating_label,
};
use infiltrator_shared::locales::{Lang, Localizer};
use lurker_core_contract::identity::CoreScope;
use lurker_core_contract::outbound::inventory::OutboundInventoryState;
use lurker_core_contract::outbound::probe::OutboundProbeBatchScope;

fn loss_badge(rating: Option<PacketLossRating>, lang: &Lang<'_>) -> Element<'static, Message> {
    let kind = match rating {
        Some(PacketLossRating::Excellent) => BadgeKind::Success,
        Some(PacketLossRating::Good) => BadgeKind::Accent,
        Some(PacketLossRating::Fair) => BadgeKind::Warning,
        Some(PacketLossRating::Poor | PacketLossRating::Dead) => BadgeKind::Danger,
        None => BadgeKind::Neutral,
    };
    badge(rating_label(rating, &Lang(lang.0)), kind)
}

fn selected_speedtest_target(state: &AppState) -> Option<SpeedtestTarget> {
    let inventory = state.runtime.outbound_inventory.data.as_ref()?;
    let scope: CoreScope = state.command_ingress()?.observed_scope()?;
    if inventory.scope() != &scope || state.runtime.runtime_selected_proxy.is_empty() {
        return None;
    }
    let OutboundInventoryState::Ready { entries, .. } = inventory.inventory() else {
        return None;
    };
    // The persisted runtime selection field carries the opaque ID value;
    // display-name matching would make duplicate labels ambiguous.
    let selected = entries
        .iter()
        .find(|entry| entry.id.value() == state.runtime.runtime_selected_proxy.as_str())?;
    SpeedtestTarget::new(
        scope,
        inventory.revision(),
        OutboundProbeBatchScope::One(selected.id.clone()),
        SpeedtestRounds::DEFAULT,
    )
    .ok()
}

pub fn speedtest_card<'a>(state: &'a AppState, lang: &Lang<'_>) -> Element<'a, Message> {
    let snapshot = &state.diag.speedtest;
    let is_running = snapshot.is_running();

    let selected_target = selected_speedtest_target(state);
    let target_label = state.runtime.runtime_selected_proxy.clone();

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
        (!is_running)
            .then(|| selected_target.clone().map(Message::StartSpeedtest))
            .flatten(),
    );

    // The live concurrency bound is read from the shared snapshot
    // and written back through the port; the UI never owns the effective fact.
    let concurrency = snapshot.concurrency.get();
    let concurrency_down = button(text("−").size(14))
        .padding([2, 10])
        .on_press(Message::AdjustSpeedtestConcurrency(-1));
    let concurrency_up = button(text("+").size(14))
        .padding([2, 10])
        .on_press(Message::AdjustSpeedtestConcurrency(1));
    let controls_row = row![
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
    let result: Option<&SpeedtestOutboundResult> = selected_target
        .as_ref()
        .and_then(|target| match target.outbound_scope() {
            OutboundProbeBatchScope::One(outbound_id) => snapshot.result(outbound_id),
            OutboundProbeBatchScope::Collection(_)
            | OutboundProbeBatchScope::Selected(_)
            | OutboundProbeBatchScope::All => None,
        })
        .or_else(|| {
            // Fall back to the fastest measured node so the card is useful even
            // before the user selects a specific target.
            snapshot.fastest()
        });

    let metric_content: Element<'_, Message> = if let Some(res) = result {
        let metrics = node_metrics(res);
        let jitter_text = metrics.jitter;
        let loss_text = metrics.loss;
        row![
            column![
                text(lang.tr("speedtest_latency").to_string())
                    .size(11)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_secondary)
                    }),
                text(
                    res.delay_ms()
                        .map(|delay| format!("{delay} ms"))
                        .unwrap_or_else(|| "—".to_owned())
                )
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
                if target_label.is_empty() {
                    "No node selected"
                } else {
                    &target_label
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

    // Fully timed-out outbound targets are shown in one explicit archive.
    let archive = dead_archive(snapshot, &Lang(lang.0));
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
    let history_lines = history_lines(snapshot, &Lang(lang.0));
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
                    if target_label.is_empty() {
                        "None"
                    } else {
                        &target_label
                    }
                ))
                .size(12)
                .font(MONO)
                .width(Length::Fill),
                // Open the typed outbound detail modal over the shared
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
            history_section,
            dead_section,
        ]
        .spacing(theme::SP_SM),
    )
}
