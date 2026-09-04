//! Latency Time-Series Sparkline and Node Stability Radar component.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::components::{badge, card, style_accent, BadgeKind};
use crate::view::svg_icons::{self, Icon};
use crate::view::theme::{self, FONT_MEDIUM, FONT_SEMIBOLD, MONO, tokens};
use crate::view::waveform::mini_waveform;
use iced::widget::{Space, button, column, row, text};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_shared::locales::{Lang, Localizer};

pub fn latency_radar_card<'a>(state: &'a AppState, lang: &Lang<'_>) -> Element<'a, Message> {
    let radar = &state.runtime.latency_radar;

    let node_name = if radar.selected_node.is_empty() {
        if !state.runtime.runtime_selected_proxy.is_empty() {
            &state.runtime.runtime_selected_proxy
        } else {
            "Select a node"
        }
    } else {
        &radar.selected_node
    };

    let sample_btn = button(
        row![
            svg_icons::icon_themed(Icon::Activity, 12.0, |t: &Theme| tokens(t).on_accent),
            Space::new().width(theme::SP_XS),
            text("Record Sample").size(11).font(FONT_MEDIUM),
        ]
        .align_y(Alignment::Center),
    )
    .padding([4, 10])
    .style(style_accent)
    .on_press(Message::RecordRadarLatencySample {
        node: node_name.to_string(),
        latency_ms: 39,
    });

    let stars = match radar.stability_score {
        5 => "★★★★★ (Tier 1)",
        4 => "★★★★☆ (Tier 2)",
        3 => "★★★☆☆ (Tier 3)",
        2 => "★★☆☆☆ (Tier 4)",
        _ => "★☆☆☆☆ (Tier 5)",
    };

    let metrics_row = row![
        column![
            text(lang.tr("latency_radar_avg").to_string()).size(11).style(|t: &Theme| text::Style { color: Some(tokens(t).text_secondary) }),
            Space::new().height(2.0),
            text(format!("{:.1} ms", radar.avg_ms)).size(14).font(FONT_SEMIBOLD).style(|t: &Theme| text::Style { color: Some(tokens(t).accent) }),
        ].width(Length::Fill),
        column![
            text(lang.tr("latency_radar_min_max").to_string()).size(11).style(|t: &Theme| text::Style { color: Some(tokens(t).text_secondary) }),
            Space::new().height(2.0),
            text(format!("{} - {} ms", radar.min_ms, radar.max_ms)).size(12).font(MONO),
        ].width(Length::Fill),
        column![
            text(lang.tr("latency_radar_score").to_string()).size(11).style(|t: &Theme| text::Style { color: Some(tokens(t).text_secondary) }),
            Space::new().height(2.0),
            badge(stars.to_string(), BadgeKind::Success),
        ].width(Length::Fill),
    ]
    .align_y(Alignment::Center);

    let sparkline = mini_waveform(&radar.samples);

    card(
        Some(lang.tr("latency_radar_title").to_string()),
        column![
            row![
                text(format!("Node: {node_name}")).size(12).font(FONT_SEMIBOLD).width(Length::Fill),
                sample_btn,
            ]
            .align_y(Alignment::Center),
            Space::new().height(theme::SP_XS),
            metrics_row,
            Space::new().height(theme::SP_XS),
            row![
                text("Time-series (last 10 samples):").size(11).style(|t: &Theme| text::Style { color: Some(tokens(t).text_secondary) }),
                Space::new().width(theme::SP_SM),
                sparkline,
            ]
            .align_y(Alignment::Center),
        ]
        .spacing(theme::SP_SM),
    )
}
