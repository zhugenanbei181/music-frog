//! DUAL-06-13: per-node speedtest detail modal.
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
use infiltrator_application::speedtest_detail_projection::{notice, project_details, summary};
use infiltrator_contract::speedtest::EgressCountryMatch;
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

fn egress_badge(matched: EgressCountryMatch, lang: &Lang<'_>) -> Element<'static, Message> {
    let (key, kind) = match matched {
        EgressCountryMatch::Match => ("speedtest_detail_match", BadgeKind::Success),
        EgressCountryMatch::Mismatch => ("speedtest_detail_mismatch", BadgeKind::Danger),
        EgressCountryMatch::Unlabelled => ("speedtest_detail_unlabelled", BadgeKind::Neutral),
        EgressCountryMatch::Unknown => ("speedtest_detail_unknown", BadgeKind::Neutral),
    };
    badge(lang.tr(key).to_string(), kind)
}

fn node_row(node: &SpeedtestDetailRow, lang: &Lang<'_>) -> Element<'static, Message> {
    container(
        column![
            row![
                text(node.node_name.clone())
                    .size(12)
                    .font(FONT_SEMIBOLD)
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
                metric(
                    lang.tr("speedtest_detail_delay").to_string(),
                    node.delay.clone()
                ),
                metric(lang.tr("speedtest_jitter").to_string(), node.jitter.clone()),
                metric(
                    lang.tr("speedtest_packet_loss").to_string(),
                    node.loss.clone()
                ),
            ]
            .spacing(8),
            row![
                metric(
                    lang.tr("speedtest_bandwidth").to_string(),
                    node.bandwidth.clone()
                ),
                metric(
                    lang.tr("speedtest_detail_stars").to_string(),
                    node.stars.clone()
                ),
            ]
            .spacing(8),
            row![
                metric(
                    lang.tr("speedtest_detail_egress").to_string(),
                    node.egress.clone()
                ),
                egress_badge(node.egress_match, lang),
            ]
            .align_y(Alignment::Center),
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

fn body(snapshot: &SpeedtestDetails, lang: &Lang<'_>, height: f32) -> Element<'static, Message> {
    let mut content = column![].spacing(6);

    if let Some(message) = notice(snapshot, &|key| lang.tr(key).into_owned()) {
        content = content.push(text(message).size(11).style(|t: &Theme| text::Style {
            color: Some(tokens(t).danger),
        }));
    }

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
            text(summary(&snapshot, &|key| lang.tr(key).into_owned()))
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
