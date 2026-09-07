//! LAN Proxy Sharing & Access Control List (ACL) component.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::components::{
    BadgeKind, badge, card, form_input_style, style_ghost, text_btn, toggle_switch,
};
use crate::view::theme::{self, FONT_SEMIBOLD, MONO, tokens};
use iced::widget::{Space, column, row, text, text_input};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_shared::locales::{Lang, Localizer};

pub fn lan_sharing_card<'a>(state: &'a AppState, lang: &Lang<'_>) -> Element<'a, Message> {
    let lan = &state.runtime.lan_sharing;

    let toggle = toggle_switch(lan.allow_lan, Message::ToggleLanSharing);

    let port_input = text_input("7890", &lan.mixed_port.to_string())
        .on_input(|val| {
            if let Ok(p) = val.parse::<u16>() {
                Message::UpdateLanSharingPort(p)
            } else {
                Message::Noop
            }
        })
        .padding([4, 8])
        .size(12)
        .font(MONO)
        .width(90)
        .style(form_input_style);

    let bind_input = text_input("* or 192.168.1.10 or [::1]", &lan.bind_address)
        .on_input(Message::UpdateLanBindAddress)
        .padding([6, 10])
        .size(12)
        .font(MONO)
        .width(Length::Fill)
        .style(form_input_style);

    card(
        Some(lang.tr("lan_sharing_title").to_string()),
        column![
            row![
                text(lang.tr("lan_sharing_desc").to_string())
                    .size(12)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_secondary)
                    })
                    .width(Length::Fill),
                toggle,
            ]
            .align_y(Alignment::Center),
            Space::new().height(theme::SP_XS),
            row![
                text(lang.tr("lan_sharing_port").to_string())
                    .size(12)
                    .font(FONT_SEMIBOLD),
                Space::new().width(theme::SP_SM),
                port_input,
                Space::new().width(theme::SP_LG),
                badge(
                    if lan.allow_lan {
                        "LAN Active"
                    } else {
                        "LAN Disabled"
                    },
                    if lan.allow_lan {
                        BadgeKind::Success
                    } else {
                        BadgeKind::Neutral
                    }
                ),
            ]
            .align_y(Alignment::Center),
            Space::new().height(theme::SP_XS),
            row![
                text(lang.tr("lan_sharing_bind").to_string())
                    .size(11)
                    .font(FONT_SEMIBOLD),
                Space::new().width(theme::SP_SM),
                bind_input,
            ]
            .align_y(Alignment::Center),
            row![
                Space::new().width(Length::Fill),
                text_btn(
                    lang.tr("lan_sharing_apply").to_string(),
                    style_ghost,
                    Some(Message::ApplyLanSharing),
                ),
            ]
            .align_y(Alignment::Center),
        ]
        .spacing(theme::SP_SM),
    )
}
