//! Speedtest & Jitter Benchmark Inspector component.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::components::{badge, card, style_accent, BadgeKind};
use crate::view::svg_icons::{self, Icon};
use crate::view::theme::{self, FONT_MEDIUM, FONT_SEMIBOLD, MONO, tokens};
use iced::widget::{Space, button, column, row, text};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_shared::locales::{Lang, Localizer};

pub fn speedtest_card<'a>(state: &'a AppState, lang: &Lang<'_>) -> Element<'a, Message> {
    let res = &state.diag.speedtest_result;

    let target = if res.target_node.is_empty() {
        state.runtime.runtime_selected_proxy.clone()
    } else {
        res.target_node.clone()
    };

    let run_btn = button(
        row![
            svg_icons::icon_themed(Icon::Zap, 14.0, |t: &Theme| tokens(t).on_accent),
            Space::new().width(theme::SP_SM),
            text(if res.is_running {
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
    .on_press_maybe((!res.is_running && !target.is_empty()).then(|| Message::RunNodeSpeedtest(target.clone())));

    let metric_content: Element<'_, Message> = if res.bandwidth_mbps > 0.0 {
        let tier_badge = match res.tier.as_str() {
            "Excellent" => badge(res.tier.clone(), BadgeKind::Success),
            "Good" => badge(res.tier.clone(), BadgeKind::Accent),
            _ => badge(res.tier.clone(), BadgeKind::Neutral),
        };

        row![
            column![
                text(lang.tr("speedtest_bandwidth").to_string()).size(11).style(|t: &Theme| text::Style { color: Some(tokens(t).text_secondary) }),
                text(format!("{:.1} Mbps", res.bandwidth_mbps)).size(16).font(FONT_SEMIBOLD).style(|t: &Theme| text::Style { color: Some(tokens(t).accent) }),
            ].width(Length::Fill),
            column![
                text(lang.tr("speedtest_jitter").to_string()).size(11).style(|t: &Theme| text::Style { color: Some(tokens(t).text_secondary) }),
                text(format!("{:.1} ms", res.jitter_ms)).size(14).font(MONO),
            ].width(Length::Fill),
            column![
                text(lang.tr("speedtest_packet_loss").to_string()).size(11).style(|t: &Theme| text::Style { color: Some(tokens(t).text_secondary) }),
                text(format!("{:.1}%", res.packet_loss_percent)).size(14).font(MONO),
            ].width(Length::Fill),
            column![
                text(lang.tr("speedtest_stability").to_string()).size(11).style(|t: &Theme| text::Style { color: Some(tokens(t).text_secondary) }),
                tier_badge,
            ],
        ]
        .align_y(Alignment::Center)
        .into()
    } else {
        row![
            text(format!("Node: {}", if target.is_empty() { "No node selected" } else { &target })).size(12).font(FONT_MEDIUM),
            Space::new().width(Length::Fill),
            text("Click to run bandwidth and packet loss benchmark").size(11).style(|t: &Theme| text::Style { color: Some(tokens(t).text_secondary) }),
        ]
        .align_y(Alignment::Center)
        .into()
    };

    card(
        Some(lang.tr("speedtest_title").to_string()),
        column![
            row![
                text(format!("Target: {}", if target.is_empty() { "None" } else { &target })).size(12).font(MONO).width(Length::Fill),
                run_btn,
            ]
            .align_y(Alignment::Center),
            Space::new().height(theme::SP_XS),
            metric_content,
        ]
        .spacing(theme::SP_SM),
    )
}
