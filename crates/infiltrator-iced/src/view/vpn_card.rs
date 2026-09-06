//! Iced projection for the Android VpnService lifecycle.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::components::{BadgeKind, badge, card, style_accent, style_ghost};
use crate::view::theme::{self, MONO, tokens};
use iced::widget::{Space, button, column, row, text};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_contract::vpn::{VpnSessionSnapshot, VpnSessionState};
use infiltrator_shared::locales::{Lang, Localizer};

pub fn vpn_card<'a>(state: &'a AppState, lang: &Lang<'_>) -> Element<'a, Message> {
    let snapshot = &state.runtime.vpn;
    let start = button(text(
        if matches!(
            &snapshot.state,
            VpnSessionState::Running | VpnSessionState::Starting
        ) {
            lang.tr("vpn_start_active")
        } else {
            lang.tr("vpn_start")
        },
    ))
    .padding([4, 12])
    .style(style_accent);
    let start = if matches!(&snapshot.state, VpnSessionState::Unsupported { .. }) {
        start
    } else {
        start.on_press(Message::StartVpn)
    };
    let stop = button(text(lang.tr("vpn_stop").to_string()))
        .padding([4, 12])
        .style(style_ghost)
        .on_press(Message::StopVpn);
    let stop = if matches!(
        &snapshot.state,
        VpnSessionState::Running | VpnSessionState::Starting | VpnSessionState::Stopping
    ) {
        stop
    } else {
        button(text(lang.tr("vpn_stopped").to_string()))
            .padding([4, 12])
            .style(style_ghost)
    };
    let details = format_details(snapshot);

    card(
        Some(lang.tr("vpn_card_title").to_string()),
        column![
            text(lang.tr("vpn_card_desc").to_string())
                .size(12)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary)
                }),
            Space::new().height(theme::SP_XS),
            row![
                badge(format_status(&snapshot.state, lang), BadgeKind::Accent),
                Space::new().width(Length::Fill),
                text(details).size(11).font(MONO),
            ]
            .align_y(Alignment::Center),
            Space::new().height(theme::SP_XS),
            row![start, Space::new().width(theme::SP_XS), stop].align_y(Alignment::Center),
        ]
        .spacing(theme::SP_SM),
    )
}

fn format_status(state: &VpnSessionState, lang: &Lang<'_>) -> String {
    match state {
        VpnSessionState::Idle => lang.tr("vpn_status_idle").to_string(),
        VpnSessionState::PermissionRequired => lang.tr("vpn_status_permission").to_string(),
        VpnSessionState::Starting => lang.tr("vpn_status_starting").to_string(),
        VpnSessionState::Running => lang.tr("vpn_status_running").to_string(),
        VpnSessionState::Stopping => lang.tr("vpn_status_stopping").to_string(),
        VpnSessionState::Stopped => lang.tr("vpn_status_stopped").to_string(),
        VpnSessionState::Revoked => lang.tr("vpn_status_revoked").to_string(),
        VpnSessionState::Unsupported { .. } => lang.tr("vpn_status_unsupported").to_string(),
        VpnSessionState::Failed { .. } => lang.tr("vpn_status_failed").to_string(),
    }
}

fn format_details(snapshot: &VpnSessionSnapshot) -> String {
    format!(
        "foreground={} · mtu={} · routes={} · ipv6={}",
        snapshot.foreground,
        snapshot
            .mtu
            .map(|value| value.to_string())
            .unwrap_or_else(|| "—".to_owned()),
        snapshot.route_count,
        snapshot.ipv6
    )
}
