//! LAN Proxy Sharing & Access Control List (ACL) component.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::component_card::card;
use crate::view::component_forms::{form_input_style, style_ghost, text_btn};
use crate::view::components::{BadgeKind, badge, toggle_switch_with_actions};
use crate::view::theme;
use crate::view::theme::{FONT_SEMIBOLD, MONO, tokens};
use iced::widget::{Space, column, row, text, text_input};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_shared::locales::{Lang, Localizer};

/// The LAN sharing status the card shows.
///
/// The status is a projection of the shared model's typed observation: a
/// readback the owner never published says "not observed", never "off".
pub fn lan_sharing_status(observed: bool, enabled: bool, lang: &Lang<'_>) -> (String, BadgeKind) {
    if !observed {
        (
            lang.tr("shell_readout_unknown").into_owned(),
            BadgeKind::Neutral,
        )
    } else if enabled {
        ("LAN Active".to_owned(), BadgeKind::Success)
    } else {
        ("LAN Disabled".to_owned(), BadgeKind::Neutral)
    }
}

pub fn lan_sharing_card<'a>(state: &'a AppState, lang: &Lang<'_>) -> Element<'a, Message> {
    let lan = state.runtime.settings_model.lan_sharing();

    // The owner publishes the LAN sharing readback as one typed observation.
    // A readback the owner never published is "unobserved", never "off".
    let observed = state.runtime.settings_model.lan_sharing_observed();
    let (status_copy, status_kind) = lan_sharing_status(observed, lan.enabled, lang);
    let available = observed && !state.runtime.runtime_patch_stage.is_applying();
    let toggle = toggle_switch_with_actions(lan.enabled, move |value| {
        available.then_some(Message::ToggleLanSharing(value))
    });

    let port_input = text_input("7890", &lan.mixed_port.to_string())
        .on_input_maybe(available.then_some(|val: String| {
            if let Ok(p) = val.parse::<u16>() {
                Message::UpdateLanSharingPort(p)
            } else {
                Message::Noop
            }
        }))
        .padding([4, 8])
        .size(12)
        .font(MONO)
        .width(90)
        .style(form_input_style);

    let bind_input = text_input("* or 192.168.1.10 or [::1]", &lan.bind_address)
        .on_input_maybe(available.then_some(Message::UpdateLanBindAddress))
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
                badge(status_copy, status_kind),
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
                    available.then_some(Message::ApplyLanSharing),
                ),
            ]
            .align_y(Alignment::Center),
        ]
        .spacing(theme::SP_SM),
    )
}
