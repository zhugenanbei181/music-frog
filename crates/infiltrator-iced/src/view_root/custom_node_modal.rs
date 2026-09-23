//! Custom Node Editor & Universal URI Codec Modal Dialog.
//!
//! DUAL-05: this modal is a *projection* of the shared
//! [`infiltrator_contract::protocol_fidelity::ProtocolStudioSnapshot`] that
//! `infiltrator-application::protocol_codec_application` publishes. Cipher
//! family (05-01), VLESS REALITY/Vision (05-02) and multiplexing parameters
//! (05-11) come from the shared draft, and the URI fidelity gaps are the ones
//! the application measured by round-tripping the draft.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::component_forms::{form_input_style, style_accent, style_ghost};
use crate::view::components::{icon_button, kbd_badge};
use crate::view::svg_icons::{self, Icon};
use crate::view::theme::{self, FONT_MEDIUM, FONT_SEMIBOLD, MONO, tokens};
use iced::widget::{Space, button, column, container, row, scrollable, text, text_input};
use iced::{Alignment, Border, Color, Element, Length, Theme, border};
use infiltrator_contract::protocol_fidelity::{ProtocolDraft, ProtocolStudioSnapshot};
use infiltrator_shared::locales::{Lang, Localizer};
fn draft_with(mut draft: ProtocolDraft, edit: impl FnOnce(&mut ProtocolDraft)) -> Message {
    edit(&mut draft);
    Message::UpdateCustomNodeDraft(Box::new(draft))
}

fn labeled_input<'a>(
    label: String,
    placeholder: &str,
    value: &str,
    width: Length,
    on_input: impl Fn(String) -> Message + 'a,
) -> Element<'a, Message> {
    column![
        text(label)
            .size(11)
            .font(FONT_SEMIBOLD)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary),
            }),
        Space::new().height(2.0),
        text_input(placeholder, value)
            .on_input(on_input)
            .padding([6, 10])
            .size(12)
            .font(MONO)
            .style(form_input_style),
    ]
    .width(width)
    .into()
}

fn chip_row<'a>(items: &[String]) -> Element<'a, Message> {
    let mut row_items: Vec<Element<'a, Message>> = Vec::new();
    for item in items {
        if !row_items.is_empty() {
            row_items.push(Space::new().width(theme::SP_XS).into());
        }
        row_items.push(kbd_badge(item.clone()));
    }
    row(row_items).spacing(theme::SP_XS).wrap().into()
}

fn fidelity_section<'a>(
    studio: &'a ProtocolStudioSnapshot,
    lang: &Lang<'_>,
) -> Element<'a, Message> {
    let Some(report) = studio.report.as_ref() else {
        return Space::new().height(0).into();
    };
    // DUAL-05-03…05-12: one shared chip list (family, cipher, flow, REALITY,
    // smux, PSK + every typed parameter chip) instead of a second vocabulary.
    let chips = report.all_chips();

    let issues = studio.issue_lines();
    let gap_template = lang.tr("custom_node_uri_gap");
    let gap_lines: Vec<String> = studio
        .uri_gaps
        .iter()
        .map(|gap| {
            infiltrator_shared::i18n_interpolator::interpolate(&gap_template, &[("field", gap)])
                .to_string()
        })
        .collect();

    let mut body = column![chip_row(&chips)].spacing(theme::SP_XS);
    for issue in issues.iter().take(6) {
        body = body.push(
            text(issue.clone())
                .size(10)
                .font(MONO)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).warning),
                }),
        );
    }
    for gap in gap_lines.iter().take(6) {
        body = body.push(
            text(gap.clone())
                .size(10)
                .font(MONO)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_tertiary),
                }),
        );
    }
    if !issues.is_empty() || !gap_lines.is_empty() {
        body = body.push(
            text(lang.tr("custom_node_issues_hint").to_string())
                .size(10)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_tertiary),
                }),
        );
    }

    container(body)
        .padding([8, 12])
        .width(Length::Fill)
        .style(|t: &Theme| {
            let tk = tokens(t);
            container::Style {
                background: Some(tk.control_bg.into()),
                border: Border {
                    radius: border::Radius::from(theme::R_CONTROL),
                    width: theme::HAIRLINE,
                    color: tk.card_border,
                },
                ..Default::default()
            }
        })
        .into()
}

pub fn custom_node_modal<'a>(state: &'a AppState) -> Element<'a, Message> {
    let lang = Lang(&state.shell.lang);
    let studio = &state.runtime.custom_node_studio;
    let fallback = ProtocolDraft::new("vless");
    let draft = studio.draft.clone().unwrap_or(fallback);
    let draft_for_name = draft.clone();
    let draft_for_server = draft.clone();
    let draft_for_port = draft.clone();
    let draft_for_type = draft.clone();
    let draft_for_secret = draft.clone();
    let draft_for_sni = draft.clone();
    let draft_for_cipher = draft.clone();
    let draft_for_flow = draft.clone();

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
        Space::new().width(theme::SP_XS),
        button(
            row![
                svg_icons::icon_themed(Icon::RefreshCw, 12.0, |t: &Theme| tokens(t).text_primary),
                Space::new().width(theme::SP_XS),
                text(lang.tr("custom_node_btn_export_uri").to_string())
                    .size(12)
                    .font(FONT_MEDIUM),
            ]
            .align_y(Alignment::Center)
        )
        .padding([6, 12])
        .style(style_ghost)
        .on_press(Message::ExportCustomNodeUri),
    ]
    .align_y(Alignment::Center);

    let name_input = labeled_input(
        lang.tr("custom_node_name").to_string(),
        "e.g. My-Vless-Node",
        &draft.name,
        Length::Fill,
        move |value| draft_with(draft_for_name.clone(), |next| next.name = value),
    );

    let type_input = labeled_input(
        lang.tr("custom_node_type").to_string(),
        "vless / ss / trojan / hysteria2 / tuic",
        &draft.node_type,
        Length::Fill,
        move |value| draft_with(draft_for_type.clone(), |next| next.node_type = value),
    );

    let row_1 = row![
        container(name_input).width(Length::FillPortion(2)),
        Space::new().width(theme::SP_SM),
        container(type_input).width(Length::FillPortion(1)),
    ]
    .align_y(Alignment::End);

    let server_input = labeled_input(
        lang.tr("custom_node_server").to_string(),
        "node.example.com",
        &draft.server,
        Length::Fill,
        move |value| draft_with(draft_for_server.clone(), |next| next.server = value),
    );

    let port_text = draft.port.to_string();
    let port_input = labeled_input(
        lang.tr("custom_node_port").to_string(),
        "443",
        &port_text,
        Length::Fill,
        move |value| {
            let parsed = value.trim().parse::<u16>().unwrap_or(0);
            draft_with(draft_for_port.clone(), |next| next.port = parsed)
        },
    );

    let row_2 = row![
        container(server_input).width(Length::FillPortion(3)),
        Space::new().width(theme::SP_SM),
        container(port_input).width(Length::FillPortion(1)),
    ]
    .align_y(Alignment::End);

    let secret_text = draft.password_or_uuid();
    let secret_input = labeled_input(
        lang.tr("custom_node_secret").to_string(),
        "password / uuid",
        &secret_text,
        Length::Fill,
        move |value| {
            draft_with(draft_for_secret.clone(), |next| {
                // One field edits the family's primary credential slot. TUIC
                // carries both `uuid` and `password`; this field edits the
                // uuid half and the shared report keeps the missing password
                // visible instead of mirroring one value into both slots.
                let slots = next.required_credentials();
                if slots.contains(&"uuid") {
                    next.uuid = value;
                } else {
                    next.password = value;
                }
            })
        },
    );

    let sni_input = labeled_input(
        lang.tr("custom_node_sni").to_string(),
        "sni.example.com",
        &draft.sni,
        Length::Fill,
        move |value| draft_with(draft_for_sni.clone(), |next| next.sni = value),
    );

    // DUAL-05-01: cipher family input (free text, typed family derived).
    let cipher_input = labeled_input(
        lang.tr("custom_node_cipher").to_string(),
        "2022-blake3-aes-256-gcm",
        &draft.cipher,
        Length::Fill,
        move |value| draft_with(draft_for_cipher.clone(), |next| next.cipher = value),
    );

    // DUAL-05-02: VLESS flow control input.
    let flow_input = labeled_input(
        lang.tr("custom_node_flow").to_string(),
        "xtls-rprx-vision",
        &draft.flow,
        Length::Fill,
        move |value| draft_with(draft_for_flow.clone(), |next| next.flow = value),
    );

    let security_row = row![
        container(cipher_input).width(Length::FillPortion(1)),
        Space::new().width(theme::SP_SM),
        container(flow_input).width(Length::FillPortion(1)),
    ]
    .align_y(Alignment::End);

    // DUAL-05-11: multiplexing parameters (YAML-only; the shared report says so).
    let mux_toggle_draft = draft.clone();
    let mux_protocol_draft = draft.clone();
    let mux_max_draft = draft.clone();
    let mux_min_draft = draft.clone();
    let mux_max_streams_draft = draft.clone();
    let mux_padding_draft = draft.clone();
    let mux_max_text = draft.smux.max_connections.to_string();
    let mux_row = row![
        column![
            text(lang.tr("custom_node_mux_enabled").to_string())
                .size(11)
                .font(FONT_SEMIBOLD)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary),
                }),
            Space::new().height(4.0),
            crate::view::components::toggle_switch(draft.smux.enabled, move |enabled| {
                draft_with(mux_toggle_draft.clone(), |next| next.smux.enabled = enabled)
            }),
        ]
        .width(Length::Fill),
        Space::new().width(theme::SP_SM),
        labeled_input(
            lang.tr("custom_node_mux_protocol").to_string(),
            "smux / yamux / h2mux",
            &draft.smux.protocol,
            Length::Fill,
            move |value| draft_with(mux_protocol_draft.clone(), |next| next.smux.protocol =
                value),
        ),
        Space::new().width(theme::SP_SM),
        labeled_input(
            lang.tr("custom_node_mux_max").to_string(),
            "4",
            &mux_max_text,
            Length::Fill,
            move |value| {
                let parsed = value.trim().parse::<u32>().unwrap_or(0);
                draft_with(mux_max_draft.clone(), |next| {
                    next.smux.max_connections = parsed
                })
            },
        ),
    ]
    .align_y(Alignment::End)
    .width(Length::Fill);

    let mux_min_text = draft.smux.min_streams.to_string();
    let mux_max_streams_text = draft.smux.max_streams.to_string();
    let mux_row_2 = row![
        labeled_input(
            lang.tr("custom_node_mux_min_streams").to_string(),
            "0",
            &mux_min_text,
            Length::Fill,
            move |value| {
                let parsed = value.trim().parse::<u32>().unwrap_or(0);
                draft_with(mux_min_draft.clone(), |next| next.smux.min_streams = parsed)
            },
        ),
        Space::new().width(theme::SP_SM),
        labeled_input(
            lang.tr("custom_node_mux_max_streams").to_string(),
            "0 = ∞",
            &mux_max_streams_text,
            Length::Fill,
            move |value| {
                let parsed = value.trim().parse::<u32>().unwrap_or(0);
                draft_with(mux_max_streams_draft.clone(), |next| {
                    next.smux.max_streams = parsed
                })
            },
        ),
        Space::new().width(theme::SP_SM),
        column![
            text(lang.tr("custom_node_mux_padding").to_string())
                .size(11)
                .font(FONT_SEMIBOLD)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary),
                }),
            Space::new().height(4.0),
            crate::view::components::toggle_switch(draft.smux.padding, move |padding| {
                draft_with(mux_padding_draft.clone(), |next| {
                    next.smux.padding = padding
                })
            }),
        ]
        .width(Length::Fill),
    ]
    .align_y(Alignment::End)
    .width(Length::Fill);

    let tls_draft = draft.clone();
    let skip_verify_draft = draft.clone();
    let tls_row = row![
        column![
            text("TLS").size(11).font(FONT_SEMIBOLD).style(|t: &Theme| {
                text::Style {
                    color: Some(tokens(t).text_secondary),
                }
            }),
            Space::new().height(4.0),
            crate::view::components::toggle_switch(draft.tls, move |tls| {
                draft_with(tls_draft.clone(), |next| next.tls = tls)
            }),
        ]
        .width(Length::FillPortion(1)),
        Space::new().width(theme::SP_SM),
        column![
            text(lang.tr("custom_node_skip_verify").to_string())
                .size(11)
                .font(FONT_SEMIBOLD)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary),
                }),
            Space::new().height(4.0),
            crate::view::components::toggle_switch(draft.skip_cert_verify, move |skip| {
                draft_with(skip_verify_draft.clone(), |next| {
                    next.skip_cert_verify = skip
                })
            }),
        ]
        .width(Length::FillPortion(1)),
        Space::new().width(theme::SP_SM),
        Space::new().width(Length::FillPortion(1)),
    ]
    .align_y(Alignment::End);

    // Shared fidelity report: protocol family, chips, issues, URI gaps.
    let fidelity = fidelity_section(studio, &lang);
    // DUAL-05-03…05-12 typed parameter editors (family-gated inside).
    let params_editor = super::custom_node_params::params_section(state);

    let export_section: Element<'_, Message> = if let Some(uri) = &studio.uri_preview {
        container(
            row![
                text(uri.clone())
                    .size(11)
                    .font(MONO)
                    .width(Length::Fill)
                    .style(|t: &Theme| text::Style {
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
                    width: theme::HAIRLINE,
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

    let body = column![
        title_row,
        Space::new().height(theme::SP_XS),
        uri_bar,
        Space::new().height(theme::SP_SM),
        fidelity,
        Space::new().height(theme::SP_SM),
        row_1,
        row_2,
        secret_input,
        sni_input,
        security_row,
        mux_row,
        mux_row_2,
        tls_row,
        params_editor,
        export_section,
        Space::new().height(theme::SP_MD),
        row![Space::new().width(Length::Fill), actions],
    ]
    .spacing(theme::SP_SM);

    let modal_card = container(scrollable(body).height(Length::Fixed(520.0)))
        .padding([20, 24])
        .width(state.shell.viewport.clamped_modal_width(620.0))
        .style(|t: &Theme| {
            let tk = tokens(t);
            container::Style {
                background: Some(tk.card_bg.into()),
                border: Border {
                    radius: border::Radius::from(theme::R_CARD),
                    width: theme::HAIRLINE,
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
            background: Some(
                Color {
                    a: 0.50,
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                }
                .into(),
            ),
            ..Default::default()
        })
        .into()
}
