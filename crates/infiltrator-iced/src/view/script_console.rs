//! Directive-DSL script sandbox console view.
//!
//! It renders the shared [`infiltrator_contract::script_sandbox::ScriptSandboxSnapshot`]
//! the application produced — the same read model the Bevy console renders.
//! There is no bundled JavaScript engine; the panel says so.

use crate::state::AppState;
use crate::types::message::Message;
use crate::types::script::ScriptAction;
use crate::view::component_card::card;
use crate::view::component_forms::style_accent;
use crate::view::components::{BadgeKind, badge, kbd_badge, modern_scrollable};
use crate::view::script_export::export_section;
use crate::view::svg_icons::Icon;
use crate::view::theme::{FONT_MEDIUM, FONT_SEMIBOLD, MONO, tokens};
use crate::view::{svg_icons, theme};
use iced::widget::{Space, button, column, container, row, text, text_editor};
use iced::{Alignment, Border, Color, Element, Length, Theme, border};
use infiltrator_application::script_application::ScriptApplication;
use infiltrator_application::script_console_projection::project_report;
use infiltrator_contract::script_export::ScriptExportKind;
use infiltrator_contract::script_run::ScriptEditorField;
use infiltrator_contract::script_sandbox::ScriptSandboxSnapshot;
use infiltrator_shared::locales::{Lang, Localizer};

fn preset_chip<'a>(
    label: String,
    preset_id: String,
    is_active: bool,
    busy: bool,
) -> Element<'a, Message> {
    button(text(label).size(12).font(FONT_MEDIUM))
        .padding([4, 10])
        .style(move |t: &Theme, status| {
            let tk = tokens(t);
            let bg = if is_active {
                tk.accent_soft
            } else {
                match status {
                    button::Status::Hovered => Color {
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
        .on_press_maybe((!busy).then_some(Message::Script(ScriptAction::SelectPreset(preset_id))))
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
    let view = project_report(snapshot, lang.0);
    let engine_label = view.engine;
    let engine_capability_label = view.capabilities;

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
        for line in view.directives {
            directives_col =
                directives_col.push(text(line).size(11).font(MONO).style(|t: &Theme| {
                    text::Style {
                        color: Some(tokens(t).text_primary),
                    }
                }));
        }
    }

    let mut logs_col = column![].spacing(4);
    for entry in view.logs {
        logs_col = logs_col.push(
            text(entry)
                .size(11)
                .font(MONO)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary),
                }),
        );
    }

    let breaker = view.breaker;
    let limits = view.limits;

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
        meta_line(lang.tr("script_sandbox_hook_stage").to_string(), view.hook,),
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

    let preset_definitions = ScriptApplication::new().builtin_presets();
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
            .push(preset_chip(
                definition.name,
                definition.id,
                is_active,
                state.editor.script_sandbox.busy(),
            ))
            .push(Space::new().width(theme::SP_SM));
    }
    let preset_row = preset_row
        .push(Space::new().width(Length::Fill))
        .push(
            button(svg_icons::icon_themed(
                Icon::Trash2,
                14.0,
                |theme: &Theme| tokens(theme).text_secondary,
            ))
            .on_press_maybe(
                (!state.editor.script_sandbox.busy())
                    .then_some(Message::Script(ScriptAction::Clear)),
            ),
        )
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
            .on_press_maybe(
                (!state.editor.script_sandbox.busy() && !state.shell.ime.is_composing())
                    .then_some(Message::Script(ScriptAction::Run)),
            ),
        );

    let script_input = column![
        text("function main(config, profile) { ... }")
            .size(12)
            .font(FONT_SEMIBOLD)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary)
            }),
        Space::new().height(theme::SP_XS),
        text_editor(&state.editor.script_code_content)
            .placeholder("function main(config, profile) { return config; }")
            .on_action(|action| Message::Script(ScriptAction::EditDocument {
                field: ScriptEditorField::Code,
                action: Box::new(action)
            }))
            .padding(10)
            .size(12)
            .font(MONO)
            .height(Length::Fixed(160.0))
    ]
    .width(Length::FillPortion(1));

    let yaml_input = column![
        text(lang.tr("script_sandbox_input_preview").to_string())
            .size(12)
            .font(FONT_SEMIBOLD)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary)
            }),
        Space::new().height(theme::SP_XS),
        text_editor(&state.editor.script_yaml_content)
            .placeholder("proxies:\n  - name: Example\n    type: ss")
            .on_action(|action| Message::Script(ScriptAction::EditDocument {
                field: ScriptEditorField::InputYaml,
                action: Box::new(action)
            }))
            .padding(10)
            .size(12)
            .font(MONO)
            .height(Length::Fixed(160.0))
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

    let mut feedback = column![].spacing(8);
    if state.editor.script_sandbox.is_running() {
        feedback = feedback.push(text(lang.tr("script_workbench_running")).size(12));
    }
    if let Some(failure) = &state.editor.script_sandbox.failure {
        feedback = feedback.push(
            container(text(&failure.message).size(12).width(Length::Fill))
                .padding(8)
                .style(|theme: &Theme| container::Style {
                    border: Border {
                        width: 1.0,
                        color: tokens(theme).danger,
                        ..Default::default()
                    },
                    ..Default::default()
                }),
        );
    }
    if state.editor.script_sandbox.can_retry() {
        feedback = feedback.push(
            button(text(lang.tr("script_workbench_retry")))
                .style(style_accent)
                .on_press(Message::Script(ScriptAction::Retry)),
        );
    }

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
                feedback,
                output_section,
                Space::new().height(theme::SP_MD),
                // DUAL-10-12: the real per-surface export (directive DSL `.js`
                // + the SHA-256 extension package), routed through the shared
                // application and the host save-file port.
                export_section(
                    state,
                    &[
                        ScriptExportKind::DirectiveDslScript,
                        ScriptExportKind::ExtensionPackageJson,
                    ],
                ),
            ]
            .spacing(theme::SP_SM),
        )
        .height(Length::Fixed(body_height)),
    );

    column![main_card].spacing(theme::SP_MD).into()
}
