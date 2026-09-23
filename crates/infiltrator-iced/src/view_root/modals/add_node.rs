//! Add-custom-node modal: the quick node form.

use super::card::{modal_backdrop, modal_card};
use crate::state::AppState;
use crate::types::message::Message;
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
            text(lang.tr("proxies_add_node_title"))
                .size(16)
                .font(crate::view::theme::FONT_SEMIBOLD)
                .style(|t: &Theme| text::Style {
                    color: Some(crate::view::theme::tokens(t).text_primary),
                }),
            Space::new().width(Length::Fill),
            button(crate::view::svg_icons::icon_themed(
                crate::view::svg_icons::Icon::X,
                14.0,
                |t: &Theme| crate::view::theme::tokens(t).text_secondary
            ))
            .padding(4)
            .style(crate::view::component_forms::style_ghost)
            .on_press(Message::OpenAddCustomNodeModal(false)),
        ]
        .align_y(Alignment::Center),
        Space::new().height(crate::view::theme::SP_SM),
        column![
            text(lang.tr("proxies_inspect_type"))
                .size(11)
                .style(|t: &Theme| text::Style {
                    color: Some(crate::view::theme::tokens(t).text_secondary),
                }),
            Space::new().height(2.0),
            pick_list(
                types,
                Some(current_type.to_string()),
                Message::UpdateNewNodeType
            )
            .width(Length::Fill)
            .style(crate::view::component_forms::form_pick_style),
        ]
        .spacing(2),
        column![
            text(lang.tr("proxies_node_name_ph"))
                .size(11)
                .style(|t: &Theme| text::Style {
                    color: Some(crate::view::theme::tokens(t).text_secondary),
                }),
            Space::new().height(2.0),
            text_input("e.g. Hong Kong 01", &state.runtime.new_node_name)
                .on_input(Message::UpdateNewNodeName)
                .padding([7, 10])
                .size(12)
                .style(crate::view::component_forms::form_input_style),
        ]
        .spacing(2),
        row![
            column![
                text(lang.tr("proxies_server_ph"))
                    .size(11)
                    .style(|t: &Theme| text::Style {
                        color: Some(crate::view::theme::tokens(t).text_secondary),
                    }),
                Space::new().height(2.0),
                text_input(
                    "e.g. 1.2.3.4 or example.com",
                    &state.runtime.new_node_server
                )
                .on_input(Message::UpdateNewNodeServer)
                .padding([7, 10])
                .size(12)
                .style(crate::view::component_forms::form_input_style),
            ]
            .width(Length::FillPortion(3)),
            Space::new().width(crate::view::theme::SP_SM),
            column![
                text(lang.tr("proxies_port_ph"))
                    .size(11)
                    .style(|t: &Theme| text::Style {
                        color: Some(crate::view::theme::tokens(t).text_secondary),
                    }),
                Space::new().height(2.0),
                text_input("443", &state.runtime.new_node_port)
                    .on_input(Message::UpdateNewNodePort)
                    .padding([7, 10])
                    .size(12)
                    .style(crate::view::component_forms::form_input_style),
            ]
            .width(Length::FillPortion(1)),
        ],
        column![
            text(lang.tr("proxies_cred_ph"))
                .size(11)
                .style(|t: &Theme| text::Style {
                    color: Some(crate::view::theme::tokens(t).text_secondary),
                }),
            Space::new().height(2.0),
            text_input("Password or UUID", &state.runtime.new_node_credential)
                .on_input(Message::UpdateNewNodeCredential)
                .padding([7, 10])
                .size(12)
                .style(crate::view::component_forms::form_input_style),
        ]
        .spacing(2),
        row![
            column![
                text(lang.tr("proxies_cipher_ph"))
                    .size(11)
                    .style(|t: &Theme| text::Style {
                        color: Some(crate::view::theme::tokens(t).text_secondary),
                    }),
                Space::new().height(2.0),
                text_input("aes-256-gcm", &state.runtime.new_node_cipher)
                    .on_input(Message::UpdateNewNodeCipher)
                    .padding([7, 10])
                    .size(12)
                    .style(crate::view::component_forms::form_input_style),
            ]
            .width(Length::FillPortion(3)),
            Space::new().width(crate::view::theme::SP_SM),
            column![
                text("TLS").size(11).style(|t: &Theme| text::Style {
                    color: Some(crate::view::theme::tokens(t).text_secondary),
                }),
                Space::new().height(6.0),
                crate::view::components::toggle_switch(
                    state.runtime.new_node_tls,
                    Message::UpdateNewNodeTls
                ),
            ]
            .width(Length::FillPortion(1)),
        ],
        Space::new().height(crate::view::theme::SP_MD),
        row![
            button(text(lang.tr("btn_cancel")).size(12))
                .padding([7, 14])
                .style(crate::view::component_forms::style_ghost)
                .on_press(Message::OpenAddCustomNodeModal(false)),
            Space::new().width(Length::Fill),
            button(text(lang.tr("proxies_add_node_btn")).size(12))
                .padding([7, 16])
                .style(crate::view::component_forms::style_accent)
                .on_press(Message::SubmitAddCustomNode),
        ]
        .align_y(Alignment::Center),
    ]
    .spacing(10);

    modal_backdrop(modal_card(form.into(), 480.0))
}
