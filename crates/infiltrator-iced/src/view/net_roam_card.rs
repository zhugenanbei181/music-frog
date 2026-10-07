//! Network Interface Roaming and Gateway Recovery component.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::component_card::card;
use crate::view::component_forms::style_accent;
use crate::view::components::{BadgeKind, badge};
use crate::view::svg_icons::Icon;
use crate::view::theme::{FONT_MEDIUM, MONO, tokens};
use crate::view::{svg_icons, theme};
use iced::widget::{Space, button, column, container, row, text};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_application::network_status_projection::{
    roaming_event, roaming_mtu, roaming_status,
};
use infiltrator_contract::network_roaming::NetworkRoamingStatus;
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
    .style(style_accent);
    let reconnect_btn = if matches!(
        &roam.status,
        NetworkRoamingStatus::Unsupported { .. } | NetworkRoamingStatus::Unknown
    ) {
        reconnect_btn
    } else {
        reconnect_btn.on_press(Message::ForceGatewayReconnect)
    };

    let active_iface = roam.active_interface.as_deref().unwrap_or("—");
    let gateway = roam.default_gateway.as_deref().unwrap_or("—");
    let mtu = roaming_mtu(roam, lang.0);
    let status = roaming_status(&roam.status, lang.0);

    let details_row = row![
        column![
            text(lang.tr("net_roam_active_iface").to_string())
                .size(11)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary)
                }),
            Space::new().height(2.0),
            row![
                badge(active_iface.to_string(), BadgeKind::Accent),
                Space::new().width(theme::SP_XS),
                badge(
                    lang.tr(if roam.active_interface.is_some() {
                        "net_roam_active_badge"
                    } else {
                        "shell_readout_unknown"
                    })
                    .to_string(),
                    if roam.active_interface.is_some() {
                        BadgeKind::Success
                    } else {
                        BadgeKind::Neutral
                    }
                ),
            ]
            .align_y(Alignment::Center),
        ]
        .width(Length::Fill),
        column![
            text(lang.tr("net_roam_gateway").to_string())
                .size(11)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary)
                }),
            Space::new().height(2.0),
            text(gateway).size(13).font(MONO),
        ]
        .width(Length::Fill),
        column![
            text(lang.tr("net_roam_mtu").to_string())
                .size(11)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary)
                }),
            Space::new().height(2.0),
            text(mtu).size(13).font(MONO),
        ]
        .width(Length::Fill),
    ]
    .align_y(Alignment::Center);

    let event_feedback: Element<'_, Message> = if let Some(ev) = &roam.last_event {
        container(
            row![
                svg_icons::icon_themed(Icon::Activity, 14.0, |t: &Theme| tokens(t).success),
                Space::new().width(theme::SP_XS),
                text(roaming_event(ev, lang.0))
                    .size(11)
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
        Some(lang.tr("net_roam_title").to_string()),
        column![
            text(lang.tr("net_roam_desc").to_string())
                .size(12)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary)
                }),
            text(status)
                .size(11)
                .font(MONO)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary)
                }),
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
