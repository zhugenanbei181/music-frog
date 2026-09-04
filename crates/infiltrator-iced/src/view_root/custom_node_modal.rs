//! Custom Node Editor & Universal URI Codec Modal Dialog.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::components::{form_input_style, icon_button, kbd_badge, style_accent, style_ghost};
use crate::view::svg_icons::{self, Icon};
use crate::view::theme::{self, FONT_MEDIUM, FONT_SEMIBOLD, MONO, tokens};
use iced::widget::{Space, button, column, container, row, text, text_input};
use iced::{Alignment, Border, Color, Element, Length, Theme, border};
use infiltrator_shared::locales::{Lang, Localizer};

pub fn custom_node_modal<'a>(state: &'a AppState) -> Element<'a, Message> {
    let lang = Lang(&state.shell.lang);

    let title_row = row![
        svg_icons::icon_themed(Icon::Plus, 18.0, |t: &Theme| tokens(t).accent),
        Space::new().width(theme::SP_SM),
        text(lang.tr("custom_node_title").to_string())
            .size(15)
            .font(FONT_SEMIBOLD)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_primary),
            }),
        Space::new().width(Length::Fill),
        icon_button(Icon::X, 14.0, Message::CloseCustomNodeModal),
    ]
    .align_y(Alignment::Center);

    // URI Quick Import Bar
    let uri_bar = row![
        text_input(
            lang.tr("custom_node_uri_placeholder").as_ref(),
            &state.runtime.custom_node_uri_input
        )
        .on_input(Message::UpdateCustomNodeUriInput)
        .padding([8, 12])
        .size(12)
        .font(MONO)
        .width(Length::Fill)
        .style(form_input_style),
        Space::new().width(theme::SP_SM),
        button(
            row![
                svg_icons::icon_themed(Icon::RefreshCw, 12.0, |t: &Theme| tokens(t).on_accent),
                Space::new().width(theme::SP_XS),
                text(lang.tr("custom_node_btn_import_uri").to_string())
                    .size(12)
                    .font(FONT_MEDIUM),
            ]
            .align_y(Alignment::Center)
        )
        .padding([6, 12])
        .style(style_accent)
        .on_press(Message::ParseAndImportCustomUri),
    ]
    .align_y(Alignment::Center);

    // Form inputs
    let name_input = column![
        text(lang.tr("custom_node_name").to_string())
            .size(11)
            .font(FONT_SEMIBOLD)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary),
            }),
        Space::new().height(2.0),
        text_input("e.g. My-Vless-Node", &state.runtime.custom_node_name_input)
            .padding([6, 10])
            .size(12)
            .style(form_input_style),
    ]
    .width(Length::FillPortion(2));

    let type_input = column![
        text(lang.tr("custom_node_type").to_string())
            .size(11)
            .font(FONT_SEMIBOLD)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary),
            }),
        Space::new().height(2.0),
        text_input("vless / ss / hysteria2 / trojan", &state.runtime.custom_node_type_input)
            .padding([6, 10])
            .size(12)
            .style(form_input_style),
    ]
    .width(Length::FillPortion(1));

    let row_1 = row![name_input, Space::new().width(theme::SP_SM), type_input];

    let server_input = column![
        text(lang.tr("custom_node_server").to_string())
            .size(11)
            .font(FONT_SEMIBOLD)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary),
            }),
        Space::new().height(2.0),
        text_input("node.example.com", &state.runtime.custom_node_server_input)
            .padding([6, 10])
            .size(12)
            .style(form_input_style),
    ]
    .width(Length::FillPortion(3));

    let port_input = column![
        text(lang.tr("custom_node_port").to_string())
            .size(11)
            .font(FONT_SEMIBOLD)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary),
            }),
        Space::new().height(2.0),
        text_input("443", &state.runtime.custom_node_port_input)
            .padding([6, 10])
            .size(12)
            .style(form_input_style),
    ]
    .width(Length::FillPortion(1));

    let row_2 = row![server_input, Space::new().width(theme::SP_SM), port_input];

    let uuid_input = column![
        text(lang.tr("custom_node_uuid_pass").to_string())
            .size(11)
            .font(FONT_SEMIBOLD)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary),
            }),
        Space::new().height(2.0),
        text_input("uuid or password", &state.runtime.custom_node_uuid_input)
            .padding([6, 10])
            .size(12)
            .font(MONO)
            .style(form_input_style),
    ]
    .width(Length::Fill);

    let sni_input = column![
        text(lang.tr("custom_node_sni").to_string())
            .size(11)
            .font(FONT_SEMIBOLD)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary),
            }),
        Space::new().height(2.0),
        text_input("sni.example.com", &state.runtime.custom_node_sni_input)
            .padding([6, 10])
            .size(12)
            .font(MONO)
            .style(form_input_style),
    ]
    .width(Length::Fill);

    let export_section: Element<'_, Message> = if let Some(uri) = &state.runtime.custom_node_exported_uri {
        container(
            row![
                text(uri.clone()).size(11).font(MONO).width(Length::Fill).style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_primary),
                }),
                Space::new().width(theme::SP_SM),
                kbd_badge("URI"),
            ]
            .align_y(Alignment::Center),
        )
        .padding([8, 12])
        .style(|t: &Theme| {
            let tk = tokens(t);
            container::Style {
                background: Some(tk.control_bg.into()),
                border: Border {
                    radius: border::Radius::from(theme::R_CONTROL),
                    width: 1.0,
                    color: tk.card_border,
                },
                ..Default::default()
            }
        })
        .into()
    } else {
        Element::from(Space::new().height(0))
    };

    let actions = row![
        button(text(lang.tr("btn_cancel").to_string()).size(12))
            .padding([6, 14])
            .style(style_ghost)
            .on_press(Message::CloseCustomNodeModal),
        Space::new().width(theme::SP_SM),
        button(text(lang.tr("btn_save").to_string()).size(12))
            .padding([6, 16])
            .style(style_accent)
            .on_press(Message::SaveCustomNodeForm),
    ]
    .align_y(Alignment::Center);

    let modal_card = container(
        column![
            title_row,
            Space::new().height(theme::SP_XS),
            uri_bar,
            Space::new().height(theme::SP_SM),
            row_1,
            row_2,
            uuid_input,
            sni_input,
            export_section,
            Space::new().height(theme::SP_MD),
            row![Space::new().width(Length::Fill), actions],
        ]
        .spacing(theme::SP_SM),
    )
    .padding([20, 24])
    .width(520)
    .style(|t: &Theme| {
        let tk = tokens(t);
        container::Style {
            background: Some(tk.card_bg.into()),
            border: Border {
                radius: border::Radius::from(theme::R_CARD),
                width: 1.0,
                color: tk.card_border,
            },
            shadow: tk.floating_shadow,
            ..Default::default()
        }
    });

    container(modal_card)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .style(|_t: &Theme| container::Style {
            background: Some(Color { a: 0.50, r: 0.0, g: 0.0, b: 0.0 }.into()),
            ..Default::default()
        })
        .into()
}
