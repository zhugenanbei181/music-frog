//! Iced adapter for live LAN CIDR ACL and HTTP Basic Authentication settings.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::component_card::card;
use crate::view::component_forms::{form_input_style, style_ghost, text_btn};
use crate::view::components::{BadgeKind, badge, toggle_switch_with_actions};
use crate::view::theme;
use crate::view::theme::{FONT_SEMIBOLD, MONO, tokens};
use iced::widget::{Space, column, row, text, text_input};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_application::settings_status_projection::format_lan_auth;
use infiltrator_contract::runtime_control::RuntimeControlStatus;
use infiltrator_shared::locales::{Lang, Localizer};

pub fn lan_security_card<'a>(state: &'a AppState, lang: &Lang<'_>) -> Element<'a, Message> {
    let security = &state.runtime.lan_security;
    let observed = &state.runtime.runtime_control;
    let available = observed.status == RuntimeControlStatus::Ready
        && observed.lan_security.is_some()
        && state.runtime.pending_runtime_patch.is_none();
    let auth_status = format_lan_auth(observed.lan_security.as_ref(), lang.0);

    let allowed = text_input("192.168.0.0/16, 10.0.0.0/8", &security.allowed_ips)
        .on_input_maybe(available.then_some(Message::UpdateLanAllowedIps))
        .padding([6, 10])
        .size(12)
        .font(MONO)
        .width(Length::Fill)
        .style(form_input_style);
    let disallowed = text_input("192.168.1.10/32", &security.disallowed_ips)
        .on_input_maybe(available.then_some(Message::UpdateLanDisallowedIps))
        .padding([6, 10])
        .size(12)
        .font(MONO)
        .width(Length::Fill)
        .style(form_input_style);
    let skip_auth = text_input("127.0.0.0/8, ::1/128", &security.skip_auth_prefixes)
        .on_input_maybe(available.then_some(Message::UpdateLanSkipAuthPrefixes))
        .padding([6, 10])
        .size(12)
        .font(MONO)
        .width(Length::Fill)
        .style(form_input_style);
    let username = text_input("musicfrog", &security.auth_username)
        .on_input_maybe(available.then_some(Message::UpdateLanAuthUsername))
        .padding([6, 10])
        .size(12)
        .font(MONO)
        .width(Length::Fill)
        .style(form_input_style);
    let password = text_input("required when enabled", &security.auth_password)
        .on_input_maybe(available.then_some(Message::UpdateLanAuthPassword))
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
                toggle_switch_with_actions(security.authentication_enabled, move |value| available
                    .then_some(Message::ToggleLanAuthentication(value))),
            ]
            .align_y(Alignment::Center),
            security_row(lang.tr("lan_security_username").to_string(), username),
            security_row(lang.tr("lan_security_password").to_string(), password),
            row![
                Space::new().width(Length::Fill),
                text_btn(
                    lang.tr("lan_security_apply").to_string(),
                    style_ghost,
                    available.then_some(Message::ApplyLanSecurity),
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
