//! Native TEA protocol editor consuming the shared field and fact projections.
use crate::state::AppState;
use crate::types::message::Message;
use crate::view::component_forms::{form_input_style, style_accent, style_ghost};
use crate::view::components::toggle_switch;
use crate::view::theme::{FONT_SEMIBOLD, MONO, tokens};
use crate::view_root::interaction_regions::InteractionRegion;
use iced::widget::{Space, button, column, container, row, scrollable, text, text_input};
use iced::{Alignment, Border, Element, Length, Theme, border};
use infiltrator_application::protocol_form::project_fields;
use infiltrator_application::protocol_studio_projection::slot_text;
use infiltrator_contract::protocol_form::ProtocolStudioSlot;
use infiltrator_shared::locales::{Lang, Localizer};

pub fn custom_node_modal(state: &AppState) -> Element<'_, Message> {
    let lang = Lang(&state.shell.lang);
    let studio = &state.runtime.custom_node_studio;
    let title = row![
        text(lang.tr("custom_node_title").into_owned())
            .font(FONT_SEMIBOLD)
            .size(16),
        Space::new().width(Length::Fill),
        button(text(lang.tr("btn_cancel").into_owned()))
            .style(style_ghost)
            .on_press(Message::CloseCustomNodeModal),
    ]
    .align_y(Alignment::Center);
    let actions = row![
        button(text(lang.tr("custom_node_btn_import_uri").into_owned()))
            .style(style_accent)
            .on_press(Message::ParseAndImportCustomUri),
        button(text(lang.tr("custom_node_btn_export_uri").into_owned()))
            .style(style_ghost)
            .on_press(Message::ExportCustomNodeUri),
        Space::new().width(Length::Fill),
        button(text(lang.tr("btn_save").into_owned()))
            .style(style_accent)
            .on_press_maybe(
                (!state.runtime.custom_node_saving).then_some(Message::SaveCustomNodeForm)
            ),
    ]
    .spacing(8)
    .align_y(Alignment::Center);
    let mut body = column![
        text_input(
            lang.tr("custom_node_uri_placeholder").as_ref(),
            &state.runtime.custom_node_uri_input
        )
        .on_input(Message::UpdateCustomNodeUriInput)
        .style(form_input_style)
        .padding([8, 12])
        .size(12),
    ]
    .spacing(10);
    if let Some(error) = &studio.last_error {
        body = body.push(text(error).size(12).style(|t: &Theme| text::Style {
            color: Some(tokens(t).warning),
        }));
    }
    if state.runtime.custom_node_saving {
        body = body.push(text(lang.tr("protocol_form_saving").into_owned()).size(12));
    }
    if let Some(draft) = &studio.draft {
        for field in project_fields(draft) {
            let label = lang.tr(field.label_key).into_owned();
            let value = state.runtime.custom_node_inputs.value(&field);
            let id = field.id;
            let input: Element<'_, Message> = if field.toggle {
                row![
                    text(label).size(12),
                    Space::new().width(Length::Fill),
                    toggle_switch(value == "true", move |on| Message::UpdateCustomNodeField(
                        id,
                        on.to_string()
                    )),
                ]
                .align_y(Alignment::Center)
                .into()
            } else {
                column![
                    text(label).size(11),
                    text_input("", &value)
                        .on_input(move |raw| Message::UpdateCustomNodeField(id, raw))
                        .style(form_input_style)
                        .padding([6, 10])
                        .size(12)
                        .font(MONO),
                ]
                .spacing(4)
                .into()
            };
            body = body.push(input);
            if let Some(error) = state.runtime.custom_node_inputs.errors.get(&id) {
                body = body.push(text(error).size(11).style(|t: &Theme| text::Style {
                    color: Some(tokens(t).warning),
                }));
            }
        }
    }
    for slot in ProtocolStudioSlot::ALL {
        body = body.push(
            text(slot_text(
                *slot,
                studio,
                |key| lang.tr(key).into_owned(),
                !matches!(state.shell.lang.as_str(), "en" | "en-US"),
            ))
            .size(11)
            .font(MONO),
        );
    }
    body = body.push(
        row![
            button(text(lang.tr("custom_node_dialer_scan").into_owned()))
                .style(style_ghost)
                .on_press(Message::ScanCustomNodeDialer),
            button(text(lang.tr("custom_node_ca_verify").into_owned()))
                .style(style_ghost)
                .on_press(Message::VerifyCustomNodeCertificateAuthority),
        ]
        .spacing(8),
    );
    let height = (state.shell.viewport.height_px - 200.0).clamp(120.0, 520.0);
    let card = container(column![title, actions, scrollable(body).height(height)].spacing(12))
        .id(InteractionRegion::Protocol.id())
        .padding(20)
        .width(state.shell.viewport.clamped_modal_width(620.0))
        .style(|t: &Theme| {
            let tk = tokens(t);
            container::Style {
                background: Some(tk.card_bg.into()),
                border: Border {
                    radius: border::Radius::from(12.0),
                    width: 1.0,
                    color: tk.card_border,
                },
                shadow: tk.floating_shadow,
                ..Default::default()
            }
        });
    container(card)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .style(|t: &Theme| container::Style {
            background: Some(tokens(t).scrim.into()),
            ..Default::default()
        })
        .into()
}
