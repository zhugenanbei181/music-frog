//! Runtime page traffic section: up/down/memory/public-IP stat cards and the
//! traffic history chart.

use crate::locales::{Lang, Localizer};
use crate::utils::format_bytes;
use crate::view::components::{TrafficChart, card, section_header, stat_card};
use crate::view::svg_icons::Icon;
use crate::view::theme::{self, SP_MD, tokens};
use crate::{AppState, Message};
use iced::widget::{Space, column, row, text};
use iced::{Element, Length, Theme};

pub(super) fn traffic_section<'a>(state: &'a AppState, lang: Lang<'a>) -> Element<'a, Message> {
    // 1. Real-time Traffic Section
    let theme_tokens = tokens(&state.shell.theme);
    let ip_stat = stat_card(
        Icon::Globe,
        lang.tr("runtime_stat_public_ip").as_ref(),
        state.diag.public_ip.as_deref().unwrap_or("—"),
        theme_tokens.accent,
        false,
    );
    let traffic_trailing = if state.diag.traffic.is_none() {
        Some(
            text(lang.tr("waiting_traffic").to_string())
                .size(11)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_tertiary),
                })
                .into(),
        )
    } else {
        None
    };

    card(
        None,
        column![
            section_header(lang.tr("overview_traffic").as_ref(), traffic_trailing),
            Space::new().height(theme::SP_MD),
            row![
                stat_card(
                    Icon::ArrowUp,
                    lang.tr("runtime_stat_up").as_ref(),
                    state
                        .diag
                        .traffic
                        .as_ref()
                        .map(|t| format_bytes(t.up))
                        .unwrap_or_else(|| "—".to_string())
                        .as_str(),
                    theme_tokens.success,
                    false,
                ),
                stat_card(
                    Icon::ArrowDown,
                    lang.tr("runtime_stat_down").as_ref(),
                    state
                        .diag
                        .traffic
                        .as_ref()
                        .map(|t| format_bytes(t.down))
                        .unwrap_or_else(|| "—".to_string())
                        .as_str(),
                    theme_tokens.accent,
                    false,
                ),
                stat_card(
                    Icon::Server,
                    lang.tr("runtime_stat_memory").as_ref(),
                    state
                        .diag
                        .memory
                        .as_ref()
                        .map(|m| format_bytes(m.in_use))
                        .unwrap_or_else(|| "—".to_string())
                        .as_str(),
                    theme_tokens.warning,
                    false,
                ),
                ip_stat,
            ]
            .spacing(SP_MD),
            Space::new().height(theme::SP_MD),
            // The chart owns its surface; it lives directly in this card like
            // the section header above (no extra frame here).
            iced::widget::Canvas::new(TrafficChart {
                history: state.diag.traffic_history.clone()
            })
            .width(Length::Fill)
            .height(Length::Fixed(110.0)),
        ],
    )
}
