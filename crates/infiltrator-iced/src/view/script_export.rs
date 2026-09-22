//! DUAL-10-12: the Iced export panel — the real per-surface export UI.
//!
//! Every button routes through [`Message::ExportScriptDraft`] into the shared
//! `ScriptExportApplication`; the panel then renders the shared
//! `ScriptExportSnapshot` (real file name, byte count, SHA-256 and the typed
//! host outcome) that the Bevy console reads from the same projection.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::components::modern_scrollable;
use crate::view::theme::{self, FONT_MEDIUM, FONT_SEMIBOLD, MONO, tokens};
use iced::widget::{Space, button, column, container, row, text};
use iced::{Alignment, Border, Color, Element, Length, Theme, border};
use infiltrator_contract::script_export::{ScriptExportKind, ScriptExportOutcome};
use infiltrator_shared::locales::{Lang, Localizer};

fn kind_button<'a>(label: String, kind: ScriptExportKind, busy: bool) -> Element<'a, Message> {
    button(text(label).size(11).font(FONT_MEDIUM))
        .padding([5, 12])
        .style(|t: &Theme, status| {
            let tk = tokens(t);
            button::Style {
                background: Some(
                    match status {
                        button::Status::Hovered => Color {
                            a: 0.16,
                            ..tk.accent
                        },
                        _ => tk.accent_soft,
                    }
                    .into(),
                ),
                border: Border {
                    radius: border::Radius::from(theme::R_CHIP),
                    width: theme::HAIRLINE,
                    color: tk.accent,
                },
                text_color: tk.accent,
                ..Default::default()
            }
        })
        .on_press_maybe((!busy).then_some(Message::ExportScriptDraft(kind)))
        .into()
}

fn detail_line<'a>(label: String, value: String, danger: bool) -> Element<'a, Message> {
    row![
        text(label)
            .size(11)
            .font(FONT_SEMIBOLD)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary),
            }),
        Space::new().width(theme::SP_SM),
        text(value)
            .size(11)
            .font(MONO)
            .style(move |t: &Theme| text::Style {
                color: Some(if danger {
                    tokens(t).danger
                } else {
                    tokens(t).text_primary
                }),
            }),
    ]
    .align_y(Alignment::Center)
    .into()
}

/// Replace `{key}` placeholders with real values.
fn fill(template: &str, pairs: &[(&str, &str)]) -> String {
    let mut out = template.to_string();
    for (key, value) in pairs {
        out = out.replace(key, value);
    }
    out
}

/// The shared export panel: buttons for the kinds this pane offers plus the
/// projection of the last export.
pub fn export_section<'a>(state: &'a AppState, kinds: &[ScriptExportKind]) -> Element<'a, Message> {
    let lang = Lang(&state.shell.lang);
    let busy = state.editor.script_sandbox.is_exporting;
    let mut buttons = row![
        text(lang.tr("script_export_title").to_string())
            .size(11)
            .font(FONT_SEMIBOLD)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary),
            }),
    ]
    .spacing(theme::SP_SM)
    .align_y(Alignment::Center);
    for kind in kinds {
        let label = lang.tr(kind.label_key()).to_string();
        buttons = buttons.push(kind_button(label, *kind, busy));
    }
    if busy {
        buttons = buttons.push(
            text(lang.tr("script_export_busy").to_string())
                .size(11)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_tertiary),
                }),
        );
    }

    let mut panel = column![buttons].spacing(theme::SP_XS);
    match state.editor.script_sandbox.export.as_ref() {
        None => {
            panel = panel.push(
                text(lang.tr("script_export_empty").to_string())
                    .size(11)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_tertiary),
                    }),
            )
        }
        Some(snapshot) => {
            panel = panel
                .push(detail_line(
                    lang.tr("script_export_kind").to_string(),
                    lang.tr(snapshot.kind.label_key()).to_string(),
                    false,
                ))
                .push(detail_line(
                    lang.tr("script_export_file").to_string(),
                    fill(
                        lang.tr("script_export_file_value").as_ref(),
                        &[
                            ("{name}", &snapshot.file_name),
                            ("{bytes}", &snapshot.byte_len().to_string()),
                        ],
                    ),
                    false,
                ))
                .push(detail_line(
                    lang.tr("script_export_outcome").to_string(),
                    snapshot.outcome_label_zh().to_string(),
                    matches!(snapshot.outcome, ScriptExportOutcome::Failed { .. }),
                ));
            match &snapshot.outcome {
                ScriptExportOutcome::Saved { path, .. } => {
                    panel = panel.push(detail_line(
                        lang.tr("script_export_saved_path").to_string(),
                        path.clone(),
                        false,
                    ));
                }
                ScriptExportOutcome::Unsupported { reason }
                | ScriptExportOutcome::Failed { reason } => {
                    panel = panel.push(detail_line(
                        lang.tr("script_export_unsupported").to_string(),
                        reason.clone(),
                        matches!(snapshot.outcome, ScriptExportOutcome::Failed { .. }),
                    ));
                }
                ScriptExportOutcome::Prepared => {}
            }
            if let Some(checksum) = snapshot.checksum.as_deref() {
                panel = panel.push(detail_line(
                    lang.tr("script_export_checksum").to_string(),
                    checksum.to_string(),
                    false,
                ));
            }
            panel = panel.push(detail_line(
                lang.tr("script_export_note").to_string(),
                snapshot.honest_note.clone(),
                false,
            ));
            panel = panel.push(
                column![
                    text(lang.tr("script_export_preview").to_string())
                        .size(11)
                        .font(FONT_SEMIBOLD)
                        .style(|t: &Theme| text::Style {
                            color: Some(tokens(t).text_secondary),
                        }),
                    container(
                        modern_scrollable(
                            text(snapshot.content_preview(1200))
                                .size(11)
                                .font(MONO)
                                .style(|t: &Theme| text::Style {
                                    color: Some(tokens(t).text_primary),
                                }),
                        )
                        .height(Length::Fixed(120.0)),
                    )
                    .padding(8)
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
                .spacing(4),
            );
        }
    }

    container(panel)
        .padding([8, 10])
        .width(Length::Fill)
        .style(|t: &Theme| {
            let tk = tokens(t);
            container::Style {
                background: Some(tk.chip_bg.into()),
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
