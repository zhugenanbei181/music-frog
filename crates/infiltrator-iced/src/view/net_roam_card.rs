//! Network Interface Roaming and Gateway Recovery component.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::components::{badge, card, style_accent, BadgeKind};
use crate::view::svg_icons::{self, Icon};
use crate::view::theme::{self, FONT_MEDIUM, MONO, tokens};
use iced::widget::{Space, button, column, container, row, text};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_shared::locales::{Lang, Localizer};

pub fn net_roam_card<'a>(state: &'a AppState, lang: &Lang<'_>) -> Element<'a, Message> {
    let roam = &state.runtime.network_roaming;

    let reconnect_btn = button(
        row![
            svg_icons::icon_themed(Icon::RefreshCw, 12.0, |t: &Theme| tokens(t).on_accent),
            Space::new().width(theme::SP_XS),
            text(lang.tr("net_roam_btn_reconnect").to_string())
                .size(11)
                .font(FONT_MEDIUM),
        ]
        .align_y(Alignment::Center),
    )
    .padding([4, 12])
    .style(style_accent)
    .on_press(Message::ForceGatewayReconnect);

    let active_iface = if roam.active_interface.is_empty() {
        "eth0"
    } else {
        &roam.active_interface
    };
    let gateway = if roam.default_gateway.is_empty() {
        "192.168.1.1"
    } else {
        &roam.default_gateway
    };
    let mtu = if roam.optimal_mtu == 0 {
        1500
    } else {
        roam.optimal_mtu
    };

    let details_row = row![
        column![
            text(lang.tr("net_roam_active_iface").to_string()).size(11).style(|t: &Theme| text::Style { color: Some(tokens(t).text_secondary) }),
            Space::new().height(2.0),
            row![
                badge(active_iface.to_string(), BadgeKind::Accent),
                Space::new().width(theme::SP_XS),
                badge("Active".to_string(), BadgeKind::Success),
            ].align_y(Alignment::Center),
        ].width(Length::Fill),
        column![
            text(lang.tr("net_roam_gateway").to_string()).size(11).style(|t: &Theme| text::Style { color: Some(tokens(t).text_secondary) }),
            Space::new().height(2.0),
            text(gateway).size(13).font(MONO),
        ].width(Length::Fill),
        column![
            text(lang.tr("net_roam_mtu").to_string()).size(11).style(|t: &Theme| text::Style { color: Some(tokens(t).text_secondary) }),
            Space::new().height(2.0),
            text(format!("{mtu} bytes")).size(13).font(MONO),
        ].width(Length::Fill),
    ]
    .align_y(Alignment::Center);

    let event_feedback: Element<'_, Message> = if let Some(ev) = &roam.last_roam_event {
        container(
            row![
                svg_icons::icon_themed(Icon::Activity, 14.0, |t: &Theme| tokens(t).success),
                Space::new().width(theme::SP_XS),
                text(ev.clone()).size(11).style(|t: &Theme| text::Style { color: Some(tokens(t).success) }),
            ]
            .align_y(Alignment::Center),
        )
        .into()
    } else {
        Element::from(Space::new().height(0))
    };

    card(
        Some(lang.tr("net_roam_title").to_string()),
        column![
            text(lang.tr("net_roam_desc").to_string()).size(12).style(|t: &Theme| text::Style { color: Some(tokens(t).text_secondary) }),
            Space::new().height(theme::SP_XS),
            details_row,
            Space::new().height(theme::SP_XS),
            row![
                event_feedback,
                Space::new().width(Length::Fill),
                reconnect_btn,
            ]
            .align_y(Alignment::Center),
        ]
        .spacing(theme::SP_SM),
    )
}
