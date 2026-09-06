//! Iced adapter for live LAN CIDR ACL and HTTP Basic Authentication settings.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::components::{
    BadgeKind, badge, card, form_input_style, style_ghost, text_btn,
};
use crate::view::theme::{self, FONT_SEMIBOLD, MONO, tokens};
use iced::widget::{Space, column, row, text, text_input};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_shared::locales::{Lang, Localizer};

pub fn lan_security_card<'a>(state: &'a AppState, lang: &Lang<'_>) -> Element<'a, Message> {
    let security = &state.runtime.lan_security;
    let auth_status = if security.authentication_enabled {
        format!(
            "{} · {}",
            lang.tr("lan_security_enabled"),
            security.authentication_user_count
        )
    } else {
        lang.tr("lan_security_disabled").to_string()
    };

    let allowed = text_input(
        "192.168.0.0/16, 10.0.0.0/8",
        &security.allowed_ips,
    )
    .on_input(Message::UpdateLanAllowedIps)
    .padding([6, 10])
    .size(12)
    .font(MONO)
    .width(Length::Fill)
    .style(form_input_style);
    let disallowed = text_input(
        "192.168.1.10/32",
        &security.disallowed_ips,
    )
    .on_input(Message::UpdateLanDisallowedIps)
    .padding([6, 10])
    .size(12)
    .font(MONO)
    .width(Length::Fill)
    .style(form_input_style);
    let skip_auth = text_input(
        "127.0.0.0/8, ::1/128",
        &security.skip_auth_prefixes,
    )
    .on_input(Message::UpdateLanSkipAuthPrefixes)
    .padding([6, 10])
    .size(12)
    .font(MONO)
    .width(Length::Fill)
    .style(form_input_style);
    let username = text_input("musicfrog", &security.auth_username)
        .on_input(Message::UpdateLanAuthUsername)
        .padding([6, 10])
        .size(12)
        .font(MONO)
        .width(Length::Fill)
        .style(form_input_style);
    let password = text_input("required when enabled", &security.auth_password)
        .on_input(Message::UpdateLanAuthPassword)
        .secure(true)
        .padding([6, 10])
        .size(12)
        .font(MONO)
        .width(Length::Fill)
        .style(form_input_style);

    card(
        Some(lang.tr("lan_security_title").to_string()),
        column![
            text(lang.tr("lan_security_desc").to_string())
                .size(12)
                .style(|theme: &Theme| text::Style {
                    color: Some(tokens(theme).text_secondary),
                }),
            Space::new().height(theme::SP_XS),
            security_row(lang.tr("lan_security_allowed").to_string(), allowed),
            security_row(lang.tr("lan_security_disallowed").to_string(), disallowed),
            security_row(lang.tr("lan_security_skip_auth").to_string(), skip_auth),
            row![
                text(lang.tr("lan_security_auth").to_string())
                    .size(12)
                    .font(FONT_SEMIBOLD),
                Space::new().width(Length::Fill),
                badge(
                    auth_status,
                    if security.authentication_enabled {
                        BadgeKind::Success
                    } else {
                        BadgeKind::Neutral
                    },
                ),
                Space::new().width(theme::SP_SM),
                crate::view::components::toggle_switch(
                    security.authentication_enabled,
                    Message::ToggleLanAuthentication,
                ),
            ]
            .align_y(Alignment::Center),
            security_row(lang.tr("lan_security_username").to_string(), username),
            security_row(lang.tr("lan_security_password").to_string(), password),
            row![
                Space::new().width(Length::Fill),
                text_btn(
                    lang.tr("lan_security_apply").to_string(),
                    style_ghost,
                    Some(Message::ApplyLanSecurity),
                ),
            ]
            .align_y(Alignment::Center),
        ]
        .spacing(theme::SP_SM),
    )
}

fn security_row<'a>(
    label: String,
    control: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    row![
        text(label).size(11).font(FONT_SEMIBOLD),
        Space::new().width(theme::SP_SM),
        control.into(),
    ]
    .align_y(Alignment::Center)
    .into()
}
