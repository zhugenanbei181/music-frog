//! Runtime page traffic section: up/down/memory/public-IP stat cards and the
//! traffic history chart.

use crate::state::AppState;
use crate::types::message::Message;
use crate::types::runtime::RuntimeStreamState;
use crate::utils::format_bytes;
use crate::view::components::{TrafficChart, card, section_header, stat_card};
use crate::view::svg_icons::Icon;
use crate::view::theme::{self, SP_MD, tokens};
use iced::widget::{Space, button, column, row, text};
use iced::{Element, Length, Theme};
use infiltrator_shared::locales::{Lang, Localizer};

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
    let ip_note = match (
        state.diag.public_ip_provider.as_deref(),
        state.diag.public_ip_checked_at.as_deref(),
        state.diag.public_ip_error.as_deref(),
    ) {
        (Some(provider), Some(checked_at), _) => {
            text(format!("经当前代理 · {provider} · {checked_at}"))
        }
        (_, _, Some(error)) => text(format!("探测失败：{error}")),
        _ => text("需手动发起出口探测（经当前代理请求 provider）".to_string()),
    }
    .size(10)
    .style(|t: &Theme| text::Style {
        color: Some(tokens(t).text_tertiary),
    });
    let ip_probe = button(text("探测出口 IP").size(10))
        .padding([4, 8])
        .style(iced::widget::button::secondary)
        .on_press(Message::FetchIpInfo);
    let ip_meta: Element<'a, Message> = column![
        ip_stat,
        row![ip_note, Space::new().width(theme::SP_SM), ip_probe].align_y(iced::Alignment::Center),
    ]
    .spacing(4)
    .into();
    let traffic_trailing: Option<Element<'a, Message>> = if state.diag.traffic.is_none() {
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
            section_header(
                lang.tr("overview_traffic").as_ref(),
                Some(
                    row![
                        stream_badge(&state.diag.traffic_stream_state),
                        Space::new().width(theme::SP_SM),
                        traffic_trailing.unwrap_or_else(|| Space::new().width(0).into()),
                    ]
                    .align_y(iced::Alignment::Center)
                    .into(),
                ),
            ),
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
                ip_meta,
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

fn stream_badge(state: &RuntimeStreamState) -> Element<'static, Message> {
    let (label, kind) = match state {
        RuntimeStreamState::Idle => ("未连接", crate::view::components::BadgeKind::Neutral),
        RuntimeStreamState::Connecting => ("连接中", crate::view::components::BadgeKind::Neutral),
        RuntimeStreamState::Connected => ("实时", crate::view::components::BadgeKind::Success),
        RuntimeStreamState::Reconnecting => ("重连中", crate::view::components::BadgeKind::Warning),
        RuntimeStreamState::Failed(_) => ("不可用", crate::view::components::BadgeKind::Danger),
    };
    crate::view::components::badge(label, kind)
}
