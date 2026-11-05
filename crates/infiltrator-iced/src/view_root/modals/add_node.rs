//! Add-custom-node modal: the quick node form.

use super::card::{modal_backdrop, modal_card};
use crate::state::AppState;
use crate::types::message::Message;
use crate::view::component_forms::{form_input_style, form_pick_style, style_accent, style_ghost};
use crate::view::components::toggle_switch;
use crate::view::svg_icons::{Icon, icon_themed};
use crate::view::theme;
use crate::view::theme::{SP_MD, SP_SM, tokens};
use iced::widget::{Space, button, column, pick_list, row, text, text_input};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_shared::locales::{Lang, Localizer};

pub fn custom_node_modal<'a>(state: &'a AppState) -> Element<'a, Message> {
    let lang = Lang(&state.shell.lang);
    let types = vec![
        "ss".into(),
        "vless".into(),
        "vmess".into(),
        "trojan".into(),
        "hysteria2".into(),
    ];
    let current_type = if state.runtime.new_node_type.is_empty() {
        "ss"
    } else {
        state.runtime.new_node_type.as_str()
    };

    let form = column![
        row![
            theme::role_body_text(lang.tr("proxies_add_node_title")).style(|t: &Theme| {
                text::Style {
                    color: Some(tokens(t).text_primary),
                }
            }),
            Space::new().width(Length::Fill),
            button(icon_themed(Icon::X, 14.0, |t: &Theme| tokens(t).text_secondary))
                .padding(4)
                .style(style_ghost)
                .on_press(Message::OpenAddCustomNodeModal(false)),
        ]
        .align_y(Alignment::Center),
        Space::new().height(SP_SM),
        column![
            theme::role_body_text(lang.tr("proxies_inspect_type")).style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary),
            }),
            Space::new().height(2.0),
            pick_list(
                types,
                Some(current_type.to_string()),
                Message::UpdateNewNodeType
            )
            .width(Length::Fill)
            .style(form_pick_style),
        ]
        .spacing(2),
        column![
            theme::role_body_text(lang.tr("proxies_node_name_ph")).style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary),
            }),
            Space::new().height(2.0),
            text_input("e.g. Hong Kong 01", &state.runtime.new_node_name)
                .on_input(Message::UpdateNewNodeName)
                .padding([7, 10])
                .style(form_input_style),
        ]
        .spacing(2),
        row![
            column![
                theme::role_body_text(lang.tr("proxies_server_ph")).style(|t: &Theme| {
                    text::Style {
                        color: Some(tokens(t).text_secondary),
                    }
                }),
                Space::new().height(2.0),
                text_input(
                    "e.g. 1.2.3.4 or example.com",
                    &state.runtime.new_node_server
                )
                .on_input(Message::UpdateNewNodeServer)
                .padding([7, 10])
                .style(form_input_style),
            ]
            .width(Length::FillPortion(3)),
            Space::new().width(SP_SM),
            column![
                theme::role_body_text(lang.tr("proxies_port_ph")).style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary),
                }),
                Space::new().height(2.0),
                text_input("443", &state.runtime.new_node_port)
                    .on_input(Message::UpdateNewNodePort)
                    .padding([7, 10])
                    .style(form_input_style),
            ]
            .width(Length::FillPortion(1)),
        ],
        column![
            theme::role_body_text(lang.tr("proxies_cred_ph")).style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary),
            }),
            Space::new().height(2.0),
            text_input("Password or UUID", &state.runtime.new_node_credential)
                .on_input(Message::UpdateNewNodeCredential)
                .padding([7, 10])
                .style(form_input_style),
        ]
        .spacing(2),
        row![
            column![
                theme::role_body_text(lang.tr("proxies_cipher_ph")).style(|t: &Theme| {
                    text::Style {
                        color: Some(tokens(t).text_secondary),
                    }
                }),
                Space::new().height(2.0),
                text_input("aes-256-gcm", &state.runtime.new_node_cipher)
                    .on_input(Message::UpdateNewNodeCipher)
                    .padding([7, 10])
                    .style(form_input_style),
            ]
            .width(Length::FillPortion(3)),
            Space::new().width(SP_SM),
            column![
                theme::role_body_text("TLS").style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary),
                }),
                Space::new().height(6.0),
                toggle_switch(state.runtime.new_node_tls, Message::UpdateNewNodeTls),
            ]
            .width(Length::FillPortion(1)),
        ],
        Space::new().height(SP_MD),
        row![
            button(theme::role_body_text(lang.tr("btn_cancel")))
                .padding([7, 14])
                .style(style_ghost)
                .on_press(Message::OpenAddCustomNodeModal(false)),
            Space::new().width(Length::Fill),
            button(theme::role_body_text(lang.tr("proxies_add_node_btn")))
                .padding([7, 16])
                .style(style_accent)
                .on_press(Message::SubmitAddCustomNode),
        ]
        .align_y(Alignment::Center),
    ]
    .spacing(10);

    modal_backdrop(modal_card(form.into(), 480.0))
}
