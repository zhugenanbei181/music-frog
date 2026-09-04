//! PCAP Packet Capture & Sniffer Inspector card.

use crate::state::AppState;
use crate::types::message::Message;
use crate::utils::format_bytes;
use crate::view::components::{badge, card, style_accent, style_danger, style_ghost, BadgeKind};
use crate::view::svg_icons::{self, Icon};
use crate::view::theme::{self, FONT_MEDIUM, MONO, tokens};
use iced::widget::{Space, button, column, container, row, text};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_shared::locales::{Lang, Localizer};

pub fn pcap_card<'a>(state: &'a AppState, lang: &Lang<'_>) -> Element<'a, Message> {
    let pcap = &state.diag.pcap_state;

    let toggle_btn = if pcap.is_capturing {
        button(
            row![
                svg_icons::icon_themed(Icon::X, 14.0, |t: &Theme| tokens(t).on_accent),
                Space::new().width(theme::SP_SM),
                text(lang.tr("pcap_btn_stop").to_string())
                    .size(12)
                    .font(FONT_MEDIUM),
            ]
            .align_y(Alignment::Center),
        )
        .padding([6, 12])
        .style(style_danger)
        .on_press(Message::TogglePcapCapture)
    } else {
        button(
            row![
                svg_icons::icon_themed(Icon::Activity, 14.0, |t: &Theme| tokens(t).on_accent),
                Space::new().width(theme::SP_SM),
                text(lang.tr("pcap_btn_start").to_string())
                    .size(12)
                    .font(FONT_MEDIUM),
            ]
            .align_y(Alignment::Center),
        )
        .padding([6, 12])
        .style(style_accent)
        .on_press(Message::TogglePcapCapture)
    };

    let export_btn = button(
        row![
            svg_icons::icon_themed(Icon::FileText, 14.0, |t: &Theme| tokens(t).text_secondary),
            Space::new().width(theme::SP_SM),
            text(lang.tr("pcap_btn_export").to_string())
                .size(12)
                .font(FONT_MEDIUM),
        ]
        .align_y(Alignment::Center),
    )
    .padding([6, 12])
    .style(style_ghost)
    .on_press_maybe((!pcap.is_capturing).then_some(Message::ExportPcapBuffer));

    let status_row = if pcap.is_capturing {
        row![
            badge("Capturing".to_string(), BadgeKind::Success),
            Space::new().width(theme::SP_SM),
            text(format!(
                "Packets: {} | Bytes: {}",
                pcap.packet_count,
                format_bytes(pcap.total_bytes as u64)
            ))
            .size(12)
            .font(MONO)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_primary)
            }),
        ]
        .align_y(Alignment::Center)
    } else {
        row![
            badge("Idle".to_string(), BadgeKind::Neutral),
            Space::new().width(theme::SP_SM),
            text(lang.tr("pcap_idle").to_string())
                .size(12)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary)
                }),
        ]
        .align_y(Alignment::Center)
    };

    let export_feedback: Element<'_, Message> = if let Some(path) = &pcap.exported_path {
        container(
            row![
                text(format!("Exported to: {path}"))
                    .size(11)
                    .font(MONO)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).success)
                    }),
            ]
            .align_y(Alignment::Center),
        )
        .into()
    } else {
        Element::from(Space::new().height(0))
    };

    card(
        Some(lang.tr("pcap_title").to_string()),
        column![
            row![
                status_row,
                Space::new().width(Length::Fill),
                toggle_btn,
                Space::new().width(theme::SP_SM),
                export_btn,
            ]
            .align_y(Alignment::Center),
            export_feedback,
        ]
        .spacing(theme::SP_SM),
    )
}
