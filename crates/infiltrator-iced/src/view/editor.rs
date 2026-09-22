//!  Editor page for raw profile YAML editing, Mixin overlay editing and
//! per-profile subscription filtering with history snapshot restoration.

use crate::state::AppState;
use crate::types::app::Route;
use crate::types::message::Message;
use crate::types::options::EditorPane;
use crate::view::components::{
    card_surface, chip, kbd_badge, segmented_control, style_accent, style_ghost,
};
use crate::view::editor_viewport;
use crate::view::svg_icons::{Icon, icon_themed};
use crate::view::theme::{self, FONT_MEDIUM, FONT_SEMIBOLD, MONO, tokens};
use iced::widget::{Row, Space, button, column, container, row, text, text_editor};
use iced::{Alignment, Border, Color, Element, Length, Theme, border};
use infiltrator_contract::yaml_snippets::YAML_SNIPPETS;
use infiltrator_shared::locales::{Lang, Localizer};
use std::path::PathBuf;

/// Truncate full SHA-256 to 8-character short hash pill string.
pub fn format_short_sha(sha: &str) -> String {
    sha.chars().take(8).collect()
}

/// Format the syntax error line badge text.
pub fn format_syntax_line_pill(line: usize) -> String {
    format!("Line {line}")
}

pub fn view(state: &AppState) -> Element<'_, Message> {
    let lang = Lang(&state.shell.lang);

    let filename = state
        .editor
        .editor_path
        .as_ref()
        .and_then(|p: &PathBuf| p.file_name())
        .and_then(|n| n.to_str())
        .map(str::to_string)
        .unwrap_or_else(|| lang.tr("editor_untitled").to_string());

    // Pane switch: raw profile YAML, the mixin overlay and the per-profile
    // subscription filter (both applied on top of the profile document).
    let pane_labels = vec![
        lang.tr("editor_pane_yaml").to_string(),
        lang.tr("editor_pane_mixin").to_string(),
        lang.tr("editor_pane_filter").to_string(),
        lang.tr("script_sandbox_title").to_string(),
    ];
    let pane_index = match state.editor.editor_pane {
        EditorPane::Profile => 0,
        EditorPane::Mixin => 1,
        EditorPane::Filter => 2,
        EditorPane::Script => 3,
    };
    let pane_switch = segmented_control(&pane_labels, pane_index, |index| {
        Message::SetEditorPane(match index {
            1 => EditorPane::Mixin,
            2 => EditorPane::Filter,
            3 => EditorPane::Script,
            _ => EditorPane::Profile,
        })
    });

    let pane_icon = match state.editor.editor_pane {
        EditorPane::Profile => Icon::FileText,
        EditorPane::Mixin => Icon::Code2,
        EditorPane::Filter => Icon::ListChecks,
        EditorPane::Script => Icon::Zap,
    };

    let pane_tag = match state.editor.editor_pane {
        EditorPane::Profile => "YAML",
        EditorPane::Mixin => "Mixin",
        EditorPane::Filter => "Filter",
        EditorPane::Script => "QuickJS",
    };

    // DUAL-09-12: the edited profile's shared write classification. A remote
    // subscription is protected unless the user explicitly unlocks it here.
    let protection = state.edited_profile_write_protection();
    let protected_blocked = protection.is_protected() && !state.editor.profile_protection_override;
    let protection_banner: Option<Element<'_, Message>> =
        (state.editor.editor_pane == EditorPane::Profile && protection.is_protected()).then(|| {
            let mut banner = row![
                icon_themed(Icon::Shield, 14.0, |t: &Theme| tokens(t).warning),
                Space::new().width(theme::SP_SM),
                column![
                    text(protection.label_zh())
                        .size(12)
                        .font(FONT_SEMIBOLD)
                        .style(|t: &Theme| text::Style {
                            color: Some(tokens(t).text_primary),
                        }),
                    text(protection.hint_zh())
                        .size(11)
                        .style(|t: &Theme| text::Style {
                            color: Some(tokens(t).text_secondary),
                        }),
                ]
                .spacing(2)
                .width(Length::Fill),
            ]
            .align_y(Alignment::Center)
            .padding([8, 12]);

            banner = banner.push(
                button(
                    text(if state.editor.profile_protection_override {
                        lang.tr("editor_protection_lock").to_string()
                    } else {
                        lang.tr("editor_protection_unlock").to_string()
                    })
                    .size(11)
                    .font(FONT_MEDIUM),
                )
                .padding([4, 10])
                .style(style_ghost)
                .on_press(Message::SetProfileProtectionOverride(
                    !state.editor.profile_protection_override,
                )),
            );
            banner = banner.push(
                button(
                    text(lang.tr("editor_protection_use_mixin").to_string())
                        .size(11)
                        .font(FONT_MEDIUM),
                )
                .padding([4, 10])
                .style(style_accent)
                .on_press(Message::SetEditorPane(EditorPane::Mixin)),
            );

            container(banner)
                .width(Length::Fill)
                .style(move |t: &Theme| {
                    let tk = tokens(t);
                    container::Style {
                        background: Some(
                            Color {
                                a: 0.12,
                                ..tk.warning
                            }
                            .into(),
                        ),
                        border: Border {
                            radius: border::Radius::from(theme::R_CONTROL),
                            width: 1.0,
                            color: tk.warning,
                        },
                        ..Default::default()
                    }
                })
                .into()
        });

    // File info block with icon chip, filename, and format chip
    let file_info = row![
        container(icon_themed(pane_icon, 16.0, |t: &Theme| tokens(t).accent))
            .width(32)
            .height(32)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .style(|t: &Theme| container::Style {
                background: Some(tokens(t).accent_soft.into()),
                border: Border {
                    radius: border::Radius::from(theme::R_CONTROL),
                    ..Default::default()
                },
                ..Default::default()
            }),
        column![
            row![
                text(filename)
                    .size(16)
                    .font(FONT_SEMIBOLD)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_primary),
                    }),
                Space::new().width(theme::SP_SM),
                chip(pane_tag),
            ]
            .align_y(Alignment::Center),
            text(
                state
                    .editor
                    .editor_path
                    .as_ref()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default()
            )
            .size(11)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_tertiary),
            }),
        ]
        .spacing(2),
    ]
    .spacing(theme::SP_MD)
    .align_y(Alignment::Center);

    // Save action button with saving state and keyboard shortcut badge
    let (save_label, saving) = match state.editor.editor_pane {
        EditorPane::Profile => (
            lang.tr("btn_save").to_string(),
            state.profile.is_saving_profile,
        ),
        EditorPane::Mixin => (
            if state.editor.is_saving_mixin {
                lang.tr("editor_applying").to_string()
            } else {
                lang.tr("editor_apply_mixin").to_string()
            },
            state.editor.is_saving_mixin,
        ),
        EditorPane::Filter => (
            if state.editor.is_saving_filter {
                lang.tr("editor_applying").to_string()
            } else {
                lang.tr("editor_apply_filter").to_string()
            },
            state.editor.is_saving_filter,
        ),
        EditorPane::Script => (
            lang.tr("script_sandbox_run").to_string(),
            state.editor.script_sandbox.is_running,
        ),
    };

    let save_btn = button(
        row![
            text(save_label).size(12).font(FONT_MEDIUM),
            Space::new().width(theme::SP_SM),
            kbd_badge("Ctrl+S"),
        ]
        .align_y(Alignment::Center),
    )
    .padding([6, 12])
    .style(style_accent)
    .on_press_maybe(
        (!saving && state.editor.editor_path.is_some() && !protected_blocked).then_some(
            match state.editor.editor_pane {
                EditorPane::Profile => Message::SaveProfile,
                EditorPane::Mixin => Message::SaveMixin,
                EditorPane::Filter => Message::SaveProfileFilter,
                EditorPane::Script => Message::RunScriptSandboxTest,
            },
        ),
    );

    let cancel_btn = button(
        row![
            icon_themed(Icon::X, 14.0, |t: &Theme| tokens(t).text_secondary),
            text(lang.tr("btn_cancel").to_string())
                .size(12)
                .font(FONT_MEDIUM),
            Space::new().width(theme::SP_XS),
            kbd_badge("Esc"),
        ]
        .spacing(theme::SP_XS)
        .align_y(Alignment::Center),
    )
    .padding([6, 12])
    .style(style_ghost)
    .on_press(Message::Navigate(Route::Profiles));

    // Toolbar row: file info + pane switch + action buttons (Save / Cancel).
    let toolbar = row![
        file_info,
        Space::new().width(Length::Fill),
        pane_switch,
        Space::new().width(theme::SP_LG),
        save_btn,
        Space::new().width(theme::SP_SM),
        cancel_btn,
    ]
    .align_y(Alignment::Center);

    // Editor area framed in a card surface, mono typeface for YAML. The
    // Mixin pane shows the overlay document; the Filter pane renders the
    // per-profile filter form instead of a text editor.
    //
    // DUAL-09-02/13: the two document panes render a *bounded window* of the
    // document with the shared line-number gutter next to it. The window is an
    // exact number of fixed-height lines, which is what lets the gutter follow
    // the widget's own scroll (see `view::editor_viewport`).
    let document_window = |content: &text_editor::Content, first_line: usize| {
        editor_viewport::viewport_for(content, state.shell.viewport.height_px, first_line)
    };
    let editor_document: Element<'_, Message> = match state.editor.editor_pane {
        EditorPane::Profile => {
            let viewport = document_window(
                &state.editor.editor_content,
                state.editor.profile_viewport.first_line(),
            );
            row![
                editor_viewport::gutter(&state.editor.editor_content, viewport),
                Space::new().width(theme::SP_XS),
                editor_viewport::editor_element(
                    &state.editor.editor_content,
                    Message::EditorAction,
                    viewport.rendered_len(),
                ),
            ]
            .into()
        }
        EditorPane::Mixin => {
            let viewport = document_window(
                &state.editor.mixin_content,
                state.editor.mixin_viewport.first_line(),
            );
            row![
                editor_viewport::gutter(&state.editor.mixin_content, viewport),
                Space::new().width(theme::SP_XS),
                editor_viewport::editor_element(
                    &state.editor.mixin_content,
                    Message::MixinEditorAction,
                    viewport.rendered_len(),
                ),
            ]
            .into()
        }
        EditorPane::Filter => crate::view::profile_filter::filter_pane(state),
        EditorPane::Script => crate::view::script_console::view(state),
    };
    let editor = container(editor_document)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(card_surface);

    // History snapshots side panel
    let history_panel = super::editor_history::history_panel(state, &lang);

    let hint_style = |t: &Theme| text::Style {
        color: Some(tokens(t).text_secondary),
    };
    let pane_hint: Option<Element<'_, Message>> = match state.editor.editor_pane {
        EditorPane::Profile => None,
        EditorPane::Mixin => Some(
            text(lang.tr("editor_mixin_hint").to_string())
                .size(11)
                .style(hint_style)
                .into(),
        ),
        EditorPane::Filter => Some(
            text(lang.tr("editor_filter_hint").to_string())
                .size(11)
                .style(hint_style)
                .into(),
        ),
        EditorPane::Script => Some(
            text(lang.tr("script_sandbox_subtitle").to_string())
                .size(11)
                .style(hint_style)
                .into(),
        ),
    };

    let syntax_alert: Option<Element<'_, Message>> =
        state.editor.syntax_error.as_ref().map(|msg| {
            let line_badge = state.editor.syntax_error_line.map(|l| {
                container(text(format_syntax_line_pill(l)).size(10).font(MONO).style(
                    |t: &Theme| text::Style {
                        color: Some(tokens(t).danger),
                    },
                ))
                .padding([2, 8])
                .style(|t: &Theme| {
                    let tk = tokens(t);
                    container::Style {
                        background: Some(
                            Color {
                                a: 0.16,
                                ..tk.danger
                            }
                            .into(),
                        ),
                        border: Border {
                            radius: border::Radius::from(theme::R_CHIP),
                            width: 1.0,
                            color: Color {
                                a: 0.35,
                                ..tk.danger
                            },
                        },
                        ..Default::default()
                    }
                })
            });

            let mut header_row = row![
                icon_themed(Icon::Activity, 16.0, |t: &Theme| tokens(t).danger),
                text(lang.tr("yaml_status_error").to_string())
                    .size(12)
                    .font(FONT_SEMIBOLD)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).danger),
                    }),
            ]
            .spacing(theme::SP_SM)
            .align_y(Alignment::Center);

            if let Some(lb) = line_badge {
                header_row = header_row.push(lb);
            }

            let banner_body = column![
                header_row,
                text(msg.clone())
                    .size(11)
                    .font(MONO)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_secondary),
                    }),
            ]
            .spacing(theme::SP_XS);

            container(banner_body)
                .padding([10, 14])
                .width(Length::Fill)
                .style(|t: &Theme| {
                    let tk = tokens(t);
                    container::Style {
                        background: Some(
                            Color {
                                a: 0.10,
                                ..tk.danger
                            }
                            .into(),
                        ),
                        border: Border {
                            radius: border::Radius::from(theme::R_CONTROL),
                            width: 1.0,
                            color: Color {
                                a: 0.30,
                                ..tk.danger
                            },
                        },
                        ..Default::default()
                    }
                })
                .into()
        });

    // DUAL-09-04: the snippet bar renders the shared catalogue — the surface
    // owns no snippet body of its own, and the same ids drive the Bevy bar.
    let mut snippet_items: Vec<Element<'_, Message>> = vec![
        text(lang.tr("yaml_snippets_title").to_string())
            .size(11)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_tertiary),
            })
            .into(),
    ];
    for snippet in YAML_SNIPPETS {
        snippet_items.push(Space::new().width(theme::SP_XS).into());
        snippet_items.push(snip_btn(lang.tr(snippet.label_key).as_ref(), snippet.id));
    }
    let snippets_bar = row![
        Row::with_children(snippet_items).align_y(Alignment::Center),
        Space::new().width(Length::Fill),
        super::editor_viewport::viewport_label(
            match state.editor.editor_pane {
                EditorPane::Mixin => document_window(
                    &state.editor.mixin_content,
                    state.editor.mixin_viewport.first_line(),
                ),
                _ => document_window(
                    &state.editor.editor_content,
                    state.editor.profile_viewport.first_line(),
                ),
            },
            &lang,
        ),
        button(
            row![
                icon_themed(Icon::Code2, 12.0, |t: &Theme| tokens(t).accent),
                Space::new().width(4.0),
                text(lang.tr("yaml_format_btn").to_string())
                    .size(11)
                    .font(FONT_MEDIUM)
            ]
            .align_y(Alignment::Center)
        )
        .style(style_ghost)
        .padding([3, 8])
        .on_press(Message::FormatYamlEditor),
    ]
    .align_y(Alignment::Center);
    // DUAL-09-14: the snippet bar belongs to the document panes only — the
    // Bevy card mounts the same shared catalogue in exactly Profile and Mixin.
    let mut content = column![toolbar, Space::new().height(theme::SP_XS)];
    if pane_has_snippet_bar(state.editor.editor_pane) {
        content = content.push(snippets_bar);
    }
    content = content.push(Space::new().height(theme::SP_SM));
    if let Some(alert) = syntax_alert {
        content = content.push(alert).push(Space::new().height(theme::SP_SM));
    }
    if let Some(hint) = pane_hint {
        content = content.push(hint).push(Space::new().height(theme::SP_SM));
    }
    if let Some(banner) = protection_banner {
        content = content.push(banner).push(Space::new().height(theme::SP_SM));
    }
    if let Some(banner) = super::editor_history::apply_banner(state, &lang) {
        content = content.push(banner).push(Space::new().height(theme::SP_SM));
    }
    // DUAL-09-14: every pane keeps the history side panel, so the snapshot
    // actions do not disappear when the Filter or Script pane is open (the
    // Bevy history card is always mounted on the page as well).
    content = content
        .push(row![editor, Space::new().width(theme::SP_MD), history_panel].height(Length::Fill));
    let content = content.spacing(theme::SP_SM);

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// DUAL-09-14: the panes that carry the shared snippet bar. The Bevy editor
/// card mounts the same catalogue in exactly these two panes, so a snippet can
/// never be inserted into a filter form.
pub fn pane_has_snippet_bar(pane: EditorPane) -> bool {
    matches!(pane, EditorPane::Profile | EditorPane::Mixin)
}

fn snip_btn<'a>(label: &str, snippet_id: &'static str) -> Element<'a, Message> {
    button(
        text(label.to_owned())
            .size(10)
            .font(MONO)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary),
            }),
    )
    .padding([2, 6])
    .style(|t: &Theme, status| {
        let tk = tokens(t);
        button::Style {
            background: match status {
                button::Status::Hovered | button::Status::Pressed => Some(tk.control_bg.into()),
                _ => Some(tk.chip_bg.into()),
            },
            border: Border {
                radius: border::Radius::from(theme::R_CHIP),
                width: 1.0,
                color: tk.card_border,
            },
            ..Default::default()
        }
    })
    .on_press(Message::InsertYamlSnippet(snippet_id))
    .into()
}

#[cfg(test)]
#[path = "../../tests/gui/view_editor_tests.rs"]
mod tests;
