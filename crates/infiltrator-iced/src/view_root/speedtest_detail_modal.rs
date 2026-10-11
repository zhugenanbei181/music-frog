//! Typed outbound speedtest detail modal.
//!
//! The modal is a pure projection of the shared [`SpeedtestSnapshot`]: every
//! row reads the same canonical result the card does, and empty / failed states
//! stay honest instead of showing fabricated metrics.

use super::modals::card::{modal_backdrop, modal_card};
use crate::state::AppState;
use crate::types::message::Message;
use crate::view::component_forms::style_ghost;
use crate::view::components::{BadgeKind, badge};
use crate::view::theme::{FONT_SEMIBOLD, HAIRLINE, MONO, R_SM, tokens};
use crate::view_root::interaction_regions::InteractionRegion;
use iced::widget::{Space, button, column, container, row, scrollable, text};
use iced::{Alignment, Border, Element, Length, Theme, border};
use infiltrator_application::speedtest_detail_projection::project_details;
use infiltrator_contract::speedtest::{SpeedtestPhase, SpeedtestResultState};
use infiltrator_contract::speedtest_details::{SpeedtestDetailRow, SpeedtestDetails};
use infiltrator_shared::locales::{Lang, Localizer};

fn metric(label: String, value: String) -> Element<'static, Message> {
    column![
        text(label).size(10).style(|t: &Theme| text::Style {
            color: Some(tokens(t).text_secondary)
        }),
        text(value).size(12).font(MONO),
    ]
    .width(Length::Fill)
    .into()
}

fn node_row(node: &SpeedtestDetailRow, lang: &Lang<'_>) -> Element<'static, Message> {
    let name = node
        .display_name
        .as_deref()
        .unwrap_or_else(|| node.outbound_id.value());
    let protocol = node.protocol_display_name.as_deref().unwrap_or("—");
    container(
        column![
            row![
                text(name.to_owned())
                    .size(12)
                    .font(FONT_SEMIBOLD)
                    .width(Length::Fill),
                text(protocol.to_owned())
                    .size(10)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_secondary)
                    }),
            ]
            .align_y(Alignment::Center),
            Space::new().height(2),
            row![
                metric(
                    lang.tr("speedtest_detail_delay").to_string(),
                    node.delay
                        .map(|delay| format!("{delay} ms"))
                        .unwrap_or_else(|| "—".to_owned())
                ),
                metric(
                    lang.tr("speedtest_jitter").to_string(),
                    node.jitter_ms
                        .map(|jitter| format!("{jitter:.1} ms"))
                        .unwrap_or_else(|| "—".to_owned()),
                ),
                metric(
                    lang.tr("speedtest_packet_loss").to_string(),
                    node.loss_percent
                        .map(|loss| format!("{loss:.1}%"))
                        .unwrap_or_else(|| "—".to_owned())
                ),
            ]
            .spacing(8),
            row![
                metric(
                    lang.tr("speedtest_detail_samples").to_string(),
                    node.sample_count.to_string()
                ),
                outcome_badge(&node.state, lang),
            ]
            .spacing(8),
        ]
        .spacing(2),
    )
    .padding([6, 8])
    .width(Length::Fill)
    .style(|theme: &Theme| container::Style {
        background: Some(tokens(theme).card_bg.into()),
        border: Border {
            radius: border::Radius::from(R_SM),
            width: HAIRLINE,
            color: tokens(theme).card_border,
        },
        ..Default::default()
    })
    .into()
}

fn outcome_badge(outcome: &SpeedtestResultState, lang: &Lang<'_>) -> Element<'static, Message> {
    let (key, kind) = match outcome {
        SpeedtestResultState::Completed => ("speedtest_result_completed", BadgeKind::Success),
        SpeedtestResultState::TimedOut => ("speedtest_result_timed_out", BadgeKind::Danger),
        SpeedtestResultState::Unsupported => ("speedtest_result_unsupported", BadgeKind::Neutral),
        SpeedtestResultState::Failed { .. } => ("speedtest_result_failed", BadgeKind::Danger),
        SpeedtestResultState::Cancelled => ("speedtest_result_cancelled", BadgeKind::Warning),
    };
    badge(lang.tr(key).to_string(), kind)
}

fn body(snapshot: &SpeedtestDetails, lang: &Lang<'_>, height: f32) -> Element<'static, Message> {
    let mut content = column![].spacing(6);

    let phase_label = match &snapshot.phase {
        SpeedtestPhase::Idle => lang.tr("speedtest_phase_idle").into_owned(),
        SpeedtestPhase::Probing => lang.tr("speedtest_phase_probing").into_owned(),
        SpeedtestPhase::CancelRequested => lang.tr("speedtest_phase_cancel_requested").into_owned(),
        SpeedtestPhase::Completed => lang.tr("speedtest_phase_completed").into_owned(),
        SpeedtestPhase::Cancelled => lang.tr("speedtest_phase_cancelled").into_owned(),
        SpeedtestPhase::Failed { failure } | SpeedtestPhase::Unavailable { failure } => {
            failure.message.clone()
        }
    };
    content = content.push(text(phase_label).size(11).style(|t: &Theme| text::Style {
        color: Some(tokens(t).text_secondary),
    }));

    let nodes = &snapshot.rows;
    if nodes.is_empty() {
        content = content.push(
            text(lang.tr("speedtest_detail_empty").to_string())
                .size(12)
                .font(MONO)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary),
                }),
        );
    } else {
        for node in nodes {
            content = content.push(node_row(node, lang));
        }
    }

    scrollable(content).height(Length::Fixed(height)).into()
}

pub(crate) fn speedtest_detail_modal(state: &AppState) -> Element<'_, Message> {
    let lang = Lang(&state.shell.lang);
    let snapshot = project_details(&state.diag.speedtest);
    let height = (state.shell.viewport.height_px - 180.0).clamp(90.0, 360.0);

    let header = row![
        column![
            text(lang.tr("speedtest_detail_title").to_string())
                .size(14)
                .font(FONT_SEMIBOLD),
            text(format!(
                "{} {}",
                snapshot.rows.len(),
                lang.tr("speedtest_detail_targets")
            ))
            .size(11)
            .font(MONO)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary)
            }),
        ]
        .width(Length::Fill),
        button(text(lang.tr("modal_close").to_string()).size(11))
            .padding([4, 10])
            .style(style_ghost)
            .on_press(Message::CloseSpeedtestDetail),
    ]
    .align_y(Alignment::Center);

    let card = column![
        header,
        Space::new().height(8),
        body(&snapshot, &lang, height)
    ]
    .spacing(4);

    modal_backdrop(modal_card(
        container(card).id(InteractionRegion::Speedtest.id()).into(),
        state.shell.viewport.detail_panel_width_px(720.0),
    ))
}
