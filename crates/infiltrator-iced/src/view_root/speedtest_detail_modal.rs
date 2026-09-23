//! DUAL-06-13: per-node speedtest detail modal.
//!
//! The modal is a pure projection of the shared [`SpeedtestSnapshot`]: every
//! row reads the same canonical result the card does, and empty / failed states
//! stay honest instead of showing fabricated metrics.

use super::modals::card::{modal_backdrop, modal_card};
use crate::state::AppState;
use crate::types::message::Message;
use crate::view::components::{BadgeKind, badge};
use crate::view::theme::{MONO, tokens};
use iced::widget::{Space, button, column, container, row, scrollable, text};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_contract::speedtest::{EgressCountryMatch, NodeSpeedtestResult, SpeedtestSnapshot};
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

fn egress_badge(matched: EgressCountryMatch, lang: &Lang<'_>) -> Element<'static, Message> {
    let (key, kind) = match matched {
        EgressCountryMatch::Match => ("speedtest_detail_match", BadgeKind::Success),
        EgressCountryMatch::Mismatch => ("speedtest_detail_mismatch", BadgeKind::Danger),
        EgressCountryMatch::Unlabelled => ("speedtest_detail_unlabelled", BadgeKind::Neutral),
        EgressCountryMatch::Unknown => ("speedtest_detail_unknown", BadgeKind::Neutral),
    };
    badge(lang.tr(key).to_string(), kind)
}

fn node_row(node: &NodeSpeedtestResult, lang: &Lang<'_>) -> Element<'static, Message> {
    let delay = node
        .delay_ms
        .map(|ms| format!("{ms} ms"))
        .unwrap_or_else(|| "—".to_string());
    let jitter = node
        .jitter
        .as_ref()
        .map(|j| format!("{:.1} ms", j.jitter_ms))
        .unwrap_or_else(|| "—".to_string());
    let loss = node
        .jitter
        .as_ref()
        .map(|j| format!("{:.1}%", j.loss_percent))
        .unwrap_or_else(|| "—".to_string());
    let bandwidth = node
        .bandwidth_mbps
        .map(|mbps| format!("{mbps:.1} Mbps"))
        .unwrap_or_else(|| "—".to_string());
    let stars = format!(
        "{}{}",
        "★".repeat(node.star_rating.min(5) as usize),
        "☆".repeat(5usize.saturating_sub(node.star_rating.min(5) as usize)),
    );

    container(
        column![
            row![
                text(node.node_name.clone())
                    .size(12)
                    .font(crate::view::theme::FONT_SEMIBOLD)
                    .width(Length::Fill),
                text(node.proxy_type.clone())
                    .size(10)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_secondary)
                    }),
            ]
            .align_y(Alignment::Center),
            Space::new().height(2),
            row![
                metric(lang.tr("speedtest_detail_delay").to_string(), delay),
                metric(lang.tr("speedtest_jitter").to_string(), jitter),
                metric(lang.tr("speedtest_packet_loss").to_string(), loss),
                metric(lang.tr("speedtest_bandwidth").to_string(), bandwidth),
                metric(lang.tr("speedtest_detail_stars").to_string(), stars),
                column![
                    text(lang.tr("speedtest_detail_egress").to_string())
                        .size(10)
                        .style(|t: &Theme| text::Style {
                            color: Some(tokens(t).text_secondary)
                        }),
                    text(node.egress_endpoint_label()).size(12).font(MONO),
                ]
                .width(Length::Fill),
                egress_badge(node.egress_country_match(), lang),
            ]
            .align_y(Alignment::Center),
        ]
        .spacing(2),
    )
    .padding([6, 8])
    .width(Length::Fill)
    .style(|theme: &Theme| container::Style {
        background: Some(tokens(theme).card_bg.into()),
        border: iced::Border {
            radius: 8.0.into(),
            width: crate::view::theme::HAIRLINE,
            color: tokens(theme).card_border,
        },
        ..Default::default()
    })
    .into()
}

fn body(snapshot: &SpeedtestSnapshot, lang: &Lang<'_>) -> Element<'static, Message> {
    let mut content = column![].spacing(6);

    if let Some(failure) = &snapshot.failure {
        content = content.push(
            text(format!("{}: {failure}", lang.tr("speedtest_detail_failed")))
                .size(11)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).danger),
                }),
        );
    }

    let mut nodes = snapshot.sorted_by_latency();
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
        for node in nodes.drain(..) {
            content = content.push(node_row(node, lang));
        }
    }

    scrollable(content).height(Length::Fixed(360.0)).into()
}

pub(crate) fn speedtest_detail_modal(state: &AppState) -> Element<'_, Message> {
    let lang = Lang(&state.shell.lang);
    let snapshot = &state.diag.speedtest;

    let header = row![
        column![
            text(lang.tr("speedtest_detail_title").to_string())
                .size(14)
                .font(crate::view::theme::FONT_SEMIBOLD),
            text(snapshot.egress_summary())
                .size(11)
                .font(MONO)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary)
                }),
        ]
        .width(Length::Fill),
        button(text(lang.tr("modal_close").to_string()).size(11))
            .padding([4, 10])
            .style(crate::view::component_forms::style_ghost)
            .on_press(Message::CloseSpeedtestDetail),
    ]
    .align_y(Alignment::Center);

    let card = column![header, Space::new().height(8), body(snapshot, &lang)].spacing(4);

    modal_backdrop(modal_card(card.into(), 720.0))
}
