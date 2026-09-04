//! QuickJS Script Sandbox Console view for testing Clash community JS extension scripts.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::components::{
    BadgeKind, badge, card, form_input_style, icon_button, kbd_badge, modern_scrollable,
    style_accent,
};
use crate::view::svg_icons::{self, Icon};
use crate::view::theme::{self, FONT_MEDIUM, FONT_SEMIBOLD, MONO, tokens};
use iced::widget::{Space, button, column, container, row, text, text_input};
use iced::{Alignment, Border, Color, Element, Length, Theme, border};
use infiltrator_shared::locales::{Lang, Localizer};

fn preset_chip<'a>(label: String, preset_id: &'static str, is_active: bool) -> Element<'a, Message> {
    button(text(label).size(12).font(FONT_MEDIUM))
        .padding([4, 10])
        .style(move |t: &Theme, status| {
            let tk = tokens(t);
            let bg = if is_active {
                tk.accent_soft
            } else {
                match status {
                    iced::widget::button::Status::Hovered => Color {
                        a: 0.12,
                        ..tk.accent
                    },
                    _ => tk.chip_bg,
                }
            };
            button::Style {
                background: Some(bg.into()),
                border: Border {
                    radius: border::Radius::from(theme::R_CHIP),
                    width: 1.0,
                    color: if is_active {
                        tk.accent
                    } else {
                        tk.card_border
                    },
                },
                text_color: if is_active {
                    tk.accent
                } else {
                    tk.text_primary
                },
                ..Default::default()
            }
        })
        .on_press(Message::SelectScriptPreset(preset_id.to_string()))
        .into()
}

pub fn view<'a>(state: &'a AppState) -> Element<'a, Message> {
    let lang = Lang(&state.shell.lang);

    let active_preset = state.editor.script_sandbox.selected_preset.as_deref();

    let preset_row = row![
        text("Presets:").size(12).font(FONT_SEMIBOLD).style(|t: &Theme| text::Style {
            color: Some(tokens(t).text_secondary)
        }),
        Space::new().width(theme::SP_SM),
        preset_chip(
            lang.tr("script_sandbox_preset_country").to_string(),
            "country",
            active_preset == Some("country")
        ),
        Space::new().width(theme::SP_SM),
        preset_chip(
            lang.tr("script_sandbox_preset_streaming").to_string(),
            "streaming",
            active_preset == Some("streaming")
        ),
        Space::new().width(theme::SP_SM),
        preset_chip(
            lang.tr("script_sandbox_preset_direct").to_string(),
            "direct",
            active_preset == Some("direct")
        ),
        Space::new().width(Length::Fill),
        icon_button(Icon::Trash2, 14.0, Message::ClearScriptSandbox),
        Space::new().width(theme::SP_SM),
        button(
            row![
                svg_icons::icon_themed(Icon::Zap, 14.0, |t: &Theme| tokens(t).on_accent),
                Space::new().width(theme::SP_SM),
                text(lang.tr("script_sandbox_run").to_string())
                    .size(12)
                    .font(FONT_MEDIUM),
                kbd_badge("Ctrl+↵")
            ]
            .align_y(Alignment::Center)
        )
        .padding([6, 14])
        .style(style_accent)
        .on_press(Message::RunScriptSandboxTest),
    ]
    .align_y(Alignment::Center);

    let script_input = column![
        text("function main(config, profile) { ... }")
            .size(12)
            .font(FONT_SEMIBOLD)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary)
            }),
        Space::new().height(theme::SP_XS),
        text_input(
            "function main(config, profile) {\n  return config;\n}",
            &state.editor.script_sandbox.script_code
        )
        .on_input(Message::UpdateScriptSandboxCode)
        .padding([10, 12])
        .size(12)
        .font(MONO)
        .width(Length::Fill)
        .style(form_input_style),
    ]
    .width(Length::FillPortion(1));

    let sample_yaml = if state.editor.script_sandbox.input_yaml.is_empty() {
        "proxies:\n  - name: Sample-Node\n    type: ss\n    server: 1.2.3.4\n    port: 8388"
    } else {
        &state.editor.script_sandbox.input_yaml
    };

    let yaml_input = column![
        text("Input Configuration (YAML):")
            .size(12)
            .font(FONT_SEMIBOLD)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary)
            }),
        Space::new().height(theme::SP_XS),
        text_input(
            "proxies:\n  - name: Example\n    type: ss",
            sample_yaml
        )
        .on_input(Message::UpdateScriptSandboxInputYaml)
        .padding([10, 12])
        .size(12)
        .font(MONO)
        .width(Length::Fill)
        .style(form_input_style),
    ]
    .width(Length::FillPortion(1));

    let editors_row = row![script_input, Space::new().width(theme::SP_MD), yaml_input];

    let output_section: Element<'_, Message> = if let Some(err) =
        &state.editor.script_sandbox.execution_error
    {
        container(
            column![
                row![
                    svg_icons::icon_themed(Icon::Shield, 16.0, |t: &Theme| tokens(t).danger),
                    Space::new().width(theme::SP_SM),
                    badge("Execution Error".to_string(), BadgeKind::Danger),
                ]
                .align_y(Alignment::Center),
                Space::new().height(theme::SP_SM),
                text(err.clone()).size(12).font(MONO).style(|t: &Theme| text::Style {
                    color: Some(tokens(t).danger)
                }),
            ]
            .spacing(theme::SP_XS),
        )
        .padding([12, 16])
        .width(Length::Fill)
        .style(|t: &Theme| {
            let tk = tokens(t);
            container::Style {
                background: Some(Color { a: 0.08, ..tk.danger }.into()),
                border: Border {
                    radius: border::Radius::from(theme::R_CONTROL),
                    width: 1.0,
                    color: Color { a: 0.30, ..tk.danger },
                },
                ..Default::default()
            }
        })
        .into()
    } else if let Some(res) = &state.editor.script_sandbox.execution_result {
        let logs_count = res.console_logs.len();
        let logs_title = format!("{}: {} logs", lang.tr("script_sandbox_logs"), logs_count);

        let mut logs_col = column![].spacing(4);
        for (idx, log_line) in res.console_logs.iter().enumerate() {
            logs_col = logs_col.push(
                text(format!("[{idx}] {log_line}"))
                    .size(11)
                    .font(MONO)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_secondary),
                    }),
            );
        }

        container(
            column![
                row![
                    svg_icons::icon_themed(Icon::Activity, 16.0, |t: &Theme| tokens(t).success),
                    Space::new().width(theme::SP_SM),
                    badge("Transform Succeeded".to_string(), BadgeKind::Success),
                    Space::new().width(theme::SP_SM),
                    kbd_badge(format!("{}ms", res.execution_time_ms)),
                ]
                .align_y(Alignment::Center),
                Space::new().height(theme::SP_SM),
                text(logs_title).size(12).font(FONT_SEMIBOLD).style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_primary)
                }),
                Space::new().height(theme::SP_XS),
                modern_scrollable(logs_col).height(Length::Fixed(80.0)),
                Space::new().height(theme::SP_SM),
                text(lang.tr("script_sandbox_output").to_string())
                    .size(12)
                    .font(FONT_SEMIBOLD)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_primary)
                    }),
                Space::new().height(theme::SP_XS),
                container(
                    modern_scrollable(
                        text(res.transformed_yaml.clone())
                            .size(11)
                            .font(MONO)
                            .style(|t: &Theme| text::Style {
                                color: Some(tokens(t).text_primary),
                            }),
                    )
                    .height(Length::Fixed(180.0)),
                )
                .padding(10)
                .width(Length::Fill)
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
                }),
            ]
            .spacing(theme::SP_XS),
        )
        .padding([14, 18])
        .width(Length::Fill)
        .style(|t: &Theme| {
            let tk = tokens(t);
            container::Style {
                background: Some(Color { a: 0.08, ..tk.success }.into()),
                border: Border {
                    radius: border::Radius::from(theme::R_CARD),
                    width: 1.0,
                    color: Color { a: 0.35, ..tk.success },
                },
                ..Default::default()
            }
        })
        .into()
    } else {
        container(
            row![
                svg_icons::icon_themed(Icon::Zap, 16.0, |t: &Theme| tokens(t).text_tertiary),
                Space::new().width(theme::SP_SM),
                text(lang.tr("script_sandbox_subtitle").to_string())
                    .size(12)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_tertiary)
                    }),
            ]
            .align_y(Alignment::Center),
        )
        .padding([12, 16])
        .width(Length::Fill)
        .style(|t: &Theme| {
            let tk = tokens(t);
            container::Style {
                background: Some(tk.control_bg.into()),
                border: Border {
                    radius: border::Radius::from(theme::R_CARD),
                    width: 1.0,
                    color: tk.card_border,
                },
                ..Default::default()
            }
        })
        .into()
    };

    let main_card = card(
        Some(lang.tr("script_sandbox_title").to_string()),
        column![
            preset_row,
            Space::new().height(theme::SP_SM),
            editors_row,
            Space::new().height(theme::SP_MD),
            output_section,
        ]
        .spacing(theme::SP_SM),
    );

    column![main_card].spacing(theme::SP_MD).into()
}
