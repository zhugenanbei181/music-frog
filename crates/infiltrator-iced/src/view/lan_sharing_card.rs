//! LAN Proxy Sharing & Access Control List (ACL) component.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::components::{badge, card, form_input_style, toggle_switch, BadgeKind};
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

    let acl_input = text_input(
        "192.168.1.0/24, 10.0.0.0/8",
        if lan.acl_whitelist_cidrs.is_empty() {
            "192.168.0.0/16, 10.0.0.0/8"
        } else {
            &lan.acl_whitelist_cidrs
        },
    )
    .on_input(Message::UpdateLanAclWhitelist)
    .padding([6, 10])
    .size(12)
    .font(MONO)
    .width(Length::Fill)
    .style(form_input_style);

    card(
        Some(lang.tr("lan_sharing_title").to_string()),
        column![
            row![
                text(lang.tr("lan_sharing_desc").to_string()).size(12).style(|t: &Theme| text::Style { color: Some(tokens(t).text_secondary) }).width(Length::Fill),
                toggle,
            ]
            .align_y(Alignment::Center),
            Space::new().height(theme::SP_XS),
            row![
                text(lang.tr("lan_sharing_port").to_string()).size(12).font(FONT_SEMIBOLD),
                Space::new().width(theme::SP_SM),
                port_input,
                Space::new().width(theme::SP_LG),
                badge(if lan.allow_lan { "LAN Active" } else { "LAN Disabled" }, if lan.allow_lan { BadgeKind::Success } else { BadgeKind::Neutral }),
            ]
            .align_y(Alignment::Center),
            Space::new().height(theme::SP_XS),
            text(lang.tr("lan_sharing_acl").to_string()).size(11).style(|t: &Theme| text::Style { color: Some(tokens(t).text_secondary) }),
            acl_input,
        ]
        .spacing(theme::SP_SM),
    )
}
