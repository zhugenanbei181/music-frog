//! Directive-DSL script sandbox console view.
//!
//! It renders the shared [`infiltrator_contract::script_sandbox::ScriptSandboxSnapshot`]
//! the application produced — the same read model the Bevy console renders.
//! There is no bundled JavaScript engine; the panel says so.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::component_forms::{form_input_style, style_accent};
use crate::view::components::{BadgeKind, badge, card, icon_button, kbd_badge, modern_scrollable};
use crate::view::svg_icons::{self, Icon};
use crate::view::theme::{self, FONT_MEDIUM, FONT_SEMIBOLD, MONO, tokens};
use iced::widget::{Space, button, column, container, row, text, text_input};
use iced::{Alignment, Border, Color, Element, Length, Theme, border};
use infiltrator_contract::script_sandbox::ScriptSandboxSnapshot;
use infiltrator_shared::locales::{Lang, Localizer};

fn preset_chip<'a>(label: String, preset_id: String, is_active: bool) -> Element<'a, Message> {
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
                    width: theme::HAIRLINE,
                    color: if is_active { tk.accent } else { tk.card_border },
                },
                text_color: if is_active {
                    tk.accent
                } else {
                    tk.text_primary
                },
                ..Default::default()
            }
        })
        .on_press(Message::SelectScriptPreset(preset_id))
        .into()
}

fn meta_line<'a>(label: String, value: String) -> Element<'a, Message> {
    row![
        text(label)
            .size(11)
            .font(FONT_SEMIBOLD)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary)
            }),
        Space::new().width(theme::SP_SM),
        text(value)
            .size(11)
            .font(MONO)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_primary)
            }),
    ]
    .align_y(Alignment::Center)
    .into()
}

/// DUAL-10-01: the engine row and the negotiated capability row. Selecting the
/// locale here keeps the surface honest in both languages while the shared read
/// model reports one engine kind; headless tests assert these exact values.
pub fn engine_meta_rows(lang: &Lang<'_>, snapshot: &ScriptSandboxSnapshot) -> (String, String) {
    let english = lang.0.starts_with("en");
    let engine = if english {
        snapshot.engine_label_en()
    } else {
        snapshot.engine_label_zh()
    };
    let capabilities = if english {
        snapshot.engine_capability_label_en()
    } else {
        snapshot.engine_capability_label_zh()
    };
    (engine.to_string(), capabilities.to_string())
}

fn preview_box<'a>(title: String, body: String) -> Element<'a, Message> {
    column![
        text(title)
            .size(12)
            .font(FONT_SEMIBOLD)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_primary)
            }),
        Space::new().height(theme::SP_XS),
        container(
            modern_scrollable(
                text(body)
                    .size(11)
                    .font(MONO)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_primary),
                    }),
            )
            .height(Length::Fixed(160.0)),
        )
        .padding(10)
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
        }),
    ]
    .width(Length::FillPortion(1))
    .into()
}

fn console_body<'a>(lang: &Lang<'_>, snapshot: &ScriptSandboxSnapshot) -> Element<'a, Message> {
    let has_error = snapshot.has_error();
    let tone = if has_error { "danger" } else { "success" };
    let title = if has_error {
        lang.tr("script_sandbox_error_title").to_string()
    } else {
        lang.tr("script_sandbox_success_title").to_string()
    };
    let kind = if has_error {
        BadgeKind::Danger
    } else {
        BadgeKind::Success
    };
    // DUAL-10-01: the engine that produced this result and its negotiated
    // capability limits, both straight from the shared read model.
    let (engine_label, engine_capability_label) = engine_meta_rows(lang, snapshot);

    let mut directives_col = column![].spacing(4);
    if snapshot.matched_directives.is_empty() {
        directives_col = directives_col.push(
            text(lang.tr("script_sandbox_no_match").to_string())
                .size(11)
                .font(MONO)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_tertiary),
                }),
        );
    } else {
        for directive in &snapshot.matched_directives {
            let line = lang
                .tr("script_sandbox_directive_row")
                .replace("{id}", &directive.id)
                .replace("{label}", &directive.label)
                .replace("{affected}", &directive.affected.to_string());
            directives_col =
                directives_col.push(text(line).size(11).font(MONO).style(|t: &Theme| {
                    text::Style {
                        color: Some(tokens(t).text_primary),
                    }
                }));
        }
    }

    let mut logs_col = column![].spacing(4);
    for entry in &snapshot.console_logs {
        logs_col = logs_col.push(
            text(format!("[{}ms] {}", entry.timestamp_ms, entry.message))
                .size(11)
                .font(MONO)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary),
                }),
        );
    }

    let breaker = lang
        .tr("script_sandbox_breaker_state")
        .replace("{state}", snapshot.circuit_breaker.label_zh())
        .replace(
            "{fails}",
            &snapshot.circuit_breaker.consecutive_failures.to_string(),
        )
        .replace(
            "{threshold}",
            &snapshot.circuit_breaker.failure_threshold.to_string(),
        )
        .replace(
            "{cooldown}",
            &snapshot.circuit_breaker.cooldown_ms.to_string(),
        )
        .replace(
            "{remaining}",
            &snapshot.circuit_breaker.remaining_cooldown_ms.to_string(),
        );
    let limits = lang
        .tr("script_sandbox_limits_value")
        .replace(
            "{memory_mb}",
            &format!(
                "{:.0}",
                snapshot.max_memory_limit_bytes as f64 / (1024.0 * 1024.0)
            ),
        )
        .replace("{timeout}", &snapshot.timeout_limit_ms.to_string())
        .replace("{elapsed}", &snapshot.execution_time_ms.to_string())
        .replace("{bytes}", &snapshot.memory_used_bytes.to_string());

    let mut body = column![
        row![
            svg_icons::icon_themed(
                if has_error {
                    Icon::Shield
                } else {
                    Icon::Activity
                },
                16.0,
                {
                    move |t: &Theme| {
                        if tone == "danger" {
                            tokens(t).danger
                        } else {
                            tokens(t).success
                        }
                    }
                }
            ),
            Space::new().width(theme::SP_SM),
            badge(title, kind),
            Space::new().width(theme::SP_SM),
            kbd_badge(format!("{}ms", snapshot.execution_time_ms)),
        ]
        .align_y(Alignment::Center),
        Space::new().height(theme::SP_XS),
        meta_line(lang.tr("script_sandbox_engine").to_string(), engine_label,),
        meta_line(
            lang.tr("script_sandbox_capabilities").to_string(),
            engine_capability_label,
        ),
        meta_line(
            lang.tr("script_sandbox_hook_stage").to_string(),
            format!("{} ({})", snapshot.hook_stage_label, snapshot.hook_stage),
        ),
        meta_line(lang.tr("script_sandbox_breaker").to_string(), breaker),
        meta_line(lang.tr("script_sandbox_limits").to_string(), limits),
    ]
    .spacing(theme::SP_XS);

    if let Some(error) = snapshot.error_detail.as_deref() {
        body = body
            .push(Space::new().height(theme::SP_SM))
            .push(
                text(error.to_string())
                    .size(12)
                    .font(MONO)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).danger),
                    }),
            )
            .push(
                text(lang.tr("script_sandbox_degraded").to_string())
                    .size(11)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_secondary),
                    }),
            );
    }

    body = body
        .push(Space::new().height(theme::SP_SM))
        .push(
            text(lang.tr("script_sandbox_matched").to_string())
                .size(12)
                .font(FONT_SEMIBOLD)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_primary),
                }),
        )
        .push(modern_scrollable(directives_col).height(Length::Fixed(72.0)))
        .push(Space::new().height(theme::SP_SM))
        .push(
            text(lang.tr("script_sandbox_logs").to_string())
                .size(12)
                .font(FONT_SEMIBOLD)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_primary),
                }),
        )
        .push(modern_scrollable(logs_col).height(Length::Fixed(96.0)))
        .push(Space::new().height(theme::SP_SM))
        .push(
            row![
                preview_box(
                    lang.tr("script_sandbox_input_preview").to_string(),
                    snapshot.input_yaml.clone(),
                ),
                Space::new().width(theme::SP_MD),
                preview_box(
                    lang.tr("script_sandbox_output").to_string(),
                    snapshot.transformed_yaml.clone().unwrap_or_default(),
                ),
            ]
            .width(Length::Fill),
        );

    container(body)
        .padding([14, 18])
        .width(Length::Fill)
        .style(move |t: &Theme| {
            let tk = tokens(t);
            let tone = if has_error { tk.danger } else { tk.success };
            container::Style {
                background: Some(Color { a: 0.08, ..tone }.into()),
                border: Border {
                    radius: border::Radius::from(theme::R_CARD),
                    width: theme::HAIRLINE,
                    color: Color { a: 0.35, ..tone },
                },
                ..Default::default()
            }
        })
        .into()
}

pub fn view<'a>(state: &'a AppState) -> Element<'a, Message> {
    let lang = Lang(&state.shell.lang);
    let snapshot = state.editor.script_sandbox.snapshot.as_ref();
    let active_preset = state.editor.script_sandbox.selected_preset.as_deref();

    let preset_definitions =
        infiltrator_application::script_application::ScriptApplication::new().builtin_presets();
    let mut preset_row = row![
        text(lang.tr("script_sandbox_presets").to_string())
            .size(12)
            .font(FONT_SEMIBOLD)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary)
            }),
        Space::new().width(theme::SP_SM),
    ]
    .align_y(Alignment::Center);
    for definition in preset_definitions {
        let is_active = active_preset == Some(definition.id.as_str());
        preset_row = preset_row
            .push(preset_chip(definition.name, definition.id, is_active))
            .push(Space::new().width(theme::SP_SM));
    }
    let preset_row = preset_row
        .push(Space::new().width(Length::Fill))
        .push(icon_button(Icon::Trash2, 14.0, Message::ClearScriptSandbox))
        .push(Space::new().width(theme::SP_SM))
        .push(
            button(
                row![
                    svg_icons::icon_themed(Icon::Zap, 14.0, |t: &Theme| tokens(t).on_accent),
                    Space::new().width(theme::SP_SM),
                    text(lang.tr("script_sandbox_run").to_string())
                        .size(12)
                        .font(FONT_MEDIUM),
                    kbd_badge("Ctrl+↵")
                ]
                .align_y(Alignment::Center),
            )
            .padding([6, 14])
            .style(style_accent)
            .on_press(Message::RunScriptSandboxTest),
        );

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
        text(lang.tr("script_sandbox_input_preview").to_string())
            .size(12)
            .font(FONT_SEMIBOLD)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary)
            }),
        Space::new().height(theme::SP_XS),
        text_input("proxies:\n  - name: Example\n    type: ss", sample_yaml)
            .on_input(Message::UpdateScriptSandboxInputYaml)
            .padding([10, 12])
            .size(12)
            .font(MONO)
            .width(Length::Fill)
            .style(form_input_style),
    ]
    .width(Length::FillPortion(1));

    let editors_row = row![script_input, Space::new().width(theme::SP_MD), yaml_input];

    let output_section: Element<'_, Message> = match snapshot {
        Some(snapshot) => console_body(&lang, snapshot),
        None => container(
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
                    width: theme::HAIRLINE,
                    color: tk.card_border,
                },
                ..Default::default()
            }
        })
        .into(),
    };

    // DUAL-10-12: the console body (editors + result + the real export
    // panel) is a bounded scroll region so the export UI stays reachable on
    // the default 780px window; the height follows the shared viewport.
    let body_height = (state.shell.viewport.height_px - 360.0).max(260.0);
    let main_card = card(
        Some(lang.tr("script_sandbox_title").to_string()),
        modern_scrollable(
            column![
                preset_row,
                Space::new().height(theme::SP_SM),
                editors_row,
                Space::new().height(theme::SP_MD),
                output_section,
                Space::new().height(theme::SP_MD),
                // DUAL-10-12: the real per-surface export (directive DSL `.js`
                // + the SHA-256 extension package), routed through the shared
                // application and the host save-file port.
                crate::view::script_export::export_section(
                    state,
                    &[
                        infiltrator_contract::script_export::ScriptExportKind::DirectiveDslScript,
                        infiltrator_contract::script_export::ScriptExportKind::ExtensionPackageJson,
                    ],
                ),
            ]
            .spacing(theme::SP_SM),
        )
        .height(Length::Fixed(body_height)),
    );

    column![main_card].spacing(theme::SP_MD).into()
}
