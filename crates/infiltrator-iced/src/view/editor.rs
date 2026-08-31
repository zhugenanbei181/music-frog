use crate::state::AppState;
use crate::types::message::Message;
use crate::types::options::EditorPane;
use crate::view::components::segmented_control;
use crate::view::svg_icons::Icon;
use crate::view::theme::{self, FONT_SEMIBOLD, MONO, R_CONTROL, tokens};
use iced::widget::{button, column, container, row, text, text_editor, Space};
use iced::{Alignment, Border, Color, Element, Length, Theme, border};
use infiltrator_shared::locales::{Lang, Localizer};
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Token-driven control styles (ui-wave2-r)
// ---------------------------------------------------------------------------

fn style_accent(t: &Theme, status: button::Status) -> button::Style {
    let tk = tokens(t);
    let (bg, fg) = match status {
        button::Status::Disabled => (tk.accent_soft, tk.accent),
        button::Status::Hovered | button::Status::Pressed => (
            Color {
                a: 0.85,
                ..tk.accent
            },
            tk.on_accent,
        ),
        _ => (tk.accent, tk.on_accent),
    };
    button::Style {
        background: Some(bg.into()),
        border: Border {
            radius: border::Radius::from(R_CONTROL),
            ..Default::default()
        },
        text_color: fg,
        ..Default::default()
    }
}

fn style_ghost(t: &Theme, status: button::Status) -> button::Style {
    let tk = tokens(t);
    button::Style {
        background: match status {
            button::Status::Hovered | button::Status::Pressed => Some(tk.control_bg.into()),
            _ => None,
        },
        border: Border {
            radius: border::Radius::from(R_CONTROL),
            width: 1.0,
            color: tk.card_border,
        },
        text_color: match status {
            button::Status::Disabled => tk.text_tertiary,
            button::Status::Hovered | button::Status::Pressed => tk.text_primary,
            _ => tk.text_secondary,
        },
        ..Default::default()
    }
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
    ];
    let pane_index = match state.editor.editor_pane {
        EditorPane::Profile => 0,
        EditorPane::Mixin => 1,
        EditorPane::Filter => 2,
    };
    let pane_switch = segmented_control(&pane_labels, pane_index, |index| {
        Message::SetEditorPane(match index {
            1 => EditorPane::Mixin,
            2 => EditorPane::Filter,
            _ => EditorPane::Profile,
        })
    });

    // Toolbar row: filename + pane switch + save (accent) / cancel (ghost).
    let toolbar = row![
        column![
            text(filename)
                .size(18)
                .font(FONT_SEMIBOLD)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_primary),
                }),
            text("YAML")
                .size(11)
                .font(theme::FONT_MEDIUM)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_tertiary),
                }),
        ]
        .spacing(2),
        pane_switch,
        iced::widget::Space::new().width(Length::Fill),
        {
            let (label, saving) = match state.editor.editor_pane {
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
            };
            button(text(label).size(12).font(theme::FONT_MEDIUM))
                .padding([7, 14])
                .style(style_accent)
                .on_press_maybe((!saving && state.editor.editor_path.is_some()).then_some(
                    match state.editor.editor_pane {
                        EditorPane::Profile => Message::SaveProfile,
                        EditorPane::Mixin => Message::SaveMixin,
                        EditorPane::Filter => Message::SaveProfileFilter,
                    },
                ))
        },
        iced::widget::Space::new().width(theme::SP_MD),
        button(
            row![
                crate::view::svg_icons::icon_themed(Icon::X, 14.0, |t: &Theme| tokens(t)
                    .text_secondary),
                text(lang.tr("btn_cancel").to_string())
                    .size(12)
                    .font(theme::FONT_MEDIUM),
            ]
            .spacing(theme::SP_SM),
        )
        .padding([7, 14])
        .style(style_ghost)
        .on_press(Message::Navigate(crate::types::app::Route::Profiles)),
    ]
    .align_y(Alignment::Center);

    // Editor area framed in a card surface, mono typeface for YAML. The
    // Mixin pane shows the overlay document; the Filter pane renders the
    // per-profile filter form instead of a text editor.
    let editor_document: Element<'_, Message> = match state.editor.editor_pane {
        EditorPane::Profile => text_editor(&state.editor.editor_content)
            .on_action(Message::EditorAction)
            .font(MONO)
            .padding(12)
            .height(Length::Fill)
            .into(),
        EditorPane::Mixin => text_editor(&state.editor.mixin_content)
            .on_action(Message::MixinEditorAction)
            .font(MONO)
            .padding(12)
            .height(Length::Fill)
            .into(),
        EditorPane::Filter => crate::view::profile_filter::filter_pane(state),
    };
    let editor = container(editor_document)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|t: &Theme| {
            let tk = tokens(t);
            container::Style {
                background: Some(tk.card_bg.into()),
                border: Border {
                    radius: border::Radius::from(theme::R_CARD),
                    width: 1.0,
                    color: tk.card_border,
                },
                shadow: tk.card_shadow,
                ..Default::default()
            }
        });

    let mut history_rows =
        column![text(lang.tr("editor_history").to_string())
            .font(theme::FONT_SEMIBOLD)
            .size(13)].spacing(theme::SP_SM);
    if state.editor.is_loading_snapshots {
        history_rows = history_rows.push(text(lang.tr("editor_history_loading").to_string()).size(11));
    } else if state.editor.profile_snapshots.is_empty() {
        history_rows = history_rows.push(
            text(lang.tr("editor_history_empty").to_string())
                .size(11)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_tertiary),
                }),
        );
    } else {
        for snapshot in state.editor.profile_snapshots.iter().take(12) {
            let short_hash: String = snapshot.sha256.chars().take(8).collect();
            history_rows = history_rows.push(
                row![
                    column![
                        text(snapshot.timestamp.format("%m-%d %H:%M").to_string()).size(11),
                        text(format!("sha {short_hash}")).size(10).font(MONO),
                    ]
                    .spacing(2)
                    .width(Length::Fill),
                    button(text(if state.editor.is_restoring_snapshot {
                        "...".to_string()
                    } else {
                        lang.tr("editor_restore").to_string()
                    }))
                    .padding([5, 8])
                    .style(style_ghost)
                    .on_press_maybe(
                        (!state.editor.is_restoring_snapshot)
                            .then_some(Message::RestoreProfileSnapshot(snapshot.path.clone())),
                    ),
                ]
                .align_y(Alignment::Center),
            );
        }
    }
    let history_panel = container(history_rows)
        .width(Length::Fixed(235.0))
        .height(Length::Fill)
        .padding(theme::SP_MD)
        .style(|t: &Theme| {
            let tk = tokens(t);
            container::Style {
                background: Some(tk.card_bg.into()),
                border: Border {
                    radius: border::Radius::from(theme::R_CARD),
                    width: 1.0,
                    color: tk.card_border,
                },
                shadow: tk.card_shadow,
                ..Default::default()
            }
        });

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
    };

    let mut content = column![toolbar, iced::widget::Space::new().height(theme::SP_MD)];
    if let Some(hint) = pane_hint {
        content = content.push(hint).push(Space::new().height(theme::SP_SM));
    }
    // The Filter pane owns its full-width form; the document panes share the
    // editor + history side panel layout.
    if state.editor.editor_pane == EditorPane::Filter {
        content = content.push(editor);
    } else {
        content = content.push(
            row![editor, iced::widget::Space::new().width(theme::SP_MD), history_panel]
                .height(Length::Fill),
        );
    }
    let content = content.spacing(theme::SP_SM);

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
