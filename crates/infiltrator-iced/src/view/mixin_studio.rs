//! DUAL-10-08/10/11: the Iced Mixin pane's shared-studio widgets.
//!
//! The preflight banner, the preset-toggle chips and the cascade pipeline
//! strip all call the shared pure functions in
//! `infiltrator_domain::mixin_studio`; the surface only renders the verdicts,
//! it never re-implements the merge or the validation.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::editor_viewport;
use crate::view::theme::{self, FONT_MEDIUM, FONT_SEMIBOLD, MONO, tokens};
use iced::widget::{button, container, row, text};
use iced::{Alignment, Border, Color, Element, Length, Theme, border};
use infiltrator_domain::mixin_studio;
use infiltrator_domain::mixin_studio::MixinColumn;
use infiltrator_shared::locales::{Lang, Localizer};

fn chip_style(enabled: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |t: &Theme, status| {
        let tk = tokens(t);
        let bg = if enabled {
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
                color: if enabled { tk.accent } else { tk.card_border },
            },
            text_color: if enabled { tk.accent } else { tk.text_primary },
            ..Default::default()
        }
    }
}

/// DUAL-10-11: the common-overlay toggle chips, state read through the shared
/// catalogue and the real mixin codec.
pub fn toggle_row(state: &AppState) -> Element<'_, Message> {
    let lang = Lang(&state.shell.lang);
    let mixin = state.editor.mixin_content.text();
    let mut chips = row![
        text(lang.tr("mixin_studio_toggles_title").to_string())
            .size(11)
            .font(FONT_SEMIBOLD)
            .style(|t: &Theme| iced::widget::text::Style {
                color: Some(tokens(t).text_secondary),
            }),
    ]
    .spacing(theme::SP_SM)
    .align_y(Alignment::Center);
    for toggle in mixin_studio::MIXIN_PRESET_TOGGLES {
        let enabled = mixin_studio::toggle_enabled(&mixin, toggle.id).unwrap_or(false);
        let button_label = lang.tr(toggle.label_key).to_string();
        chips = chips.push(
            button(text(button_label).size(11).font(FONT_MEDIUM))
                .padding([4, 10])
                .style(chip_style(enabled))
                .on_press(Message::ToggleMixinPreset(toggle.id.to_string(), !enabled)),
        );
    }
    chips.into()
}

/// DUAL-10-10: the shared preflight verdict for the current overlay. A
/// blocking verdict states the real reason the save is refused.
pub fn preflight_banner(state: &AppState) -> Element<'_, Message> {
    let lang = Lang(&state.shell.lang);
    let base = state.editor.editor_content.text();
    let report = mixin_studio::preflight_mixin(&base, &state.editor.mixin_content.text());
    let danger = !report.valid;
    let headline = if report.valid {
        lang.tr("mixin_studio_preflight_ok").to_string()
    } else {
        lang.tr("mixin_studio_preflight_blocked").to_string()
    };
    let detail = if report.valid {
        lang.tr("mixin_studio_preflight_hint").to_string()
    } else {
        report.error.clone().unwrap_or_default()
    };
    container(
        iced::widget::column![
            text(headline)
                .size(11)
                .font(FONT_SEMIBOLD)
                .style(move |t: &Theme| iced::widget::text::Style {
                    color: Some(if danger {
                        tokens(t).danger
                    } else {
                        tokens(t).success
                    })
                }),
            text(detail)
                .size(11)
                .font(FONT_MEDIUM)
                .style(|t: &Theme| iced::widget::text::Style {
                    color: Some(tokens(t).text_secondary),
                }),
        ]
        .spacing(theme::SP_XS),
    )
    .padding([6, 10])
    .width(Length::Fill)
    .style(move |t: &Theme| {
        let accent = if danger {
            tokens(t).danger
        } else {
            tokens(t).success
        };
        container::Style {
            background: Some(Color { a: 0.08, ..accent }.into()),
            border: Border {
                radius: border::Radius::from(theme::R_CONTROL),
                width: theme::HAIRLINE,
                color: Color { a: 0.30, ..accent },
            },
            ..Default::default()
        }
    })
    .into()
}

/// DUAL-10-08: the cascade overlay pipeline strip. Each stage's line count is
/// the real output of a pipeline run with exactly those stages.
pub fn cascade_strip(state: &AppState) -> Element<'_, Message> {
    let lang = Lang(&state.shell.lang);
    let base = state.editor.editor_content.text();
    let mixin_config = serde_yaml_ng::from_str::<infiltrator_domain::mixin::MixinConfig>(
        &state.editor.mixin_content.text(),
    )
    .ok();
    let report = mixin_studio::preview_cascade(&base, None, None, mixin_config.as_ref(), None);

    let mut stages = row![
        text(lang.tr("mixin_studio_cascade_title").to_string())
            .size(11)
            .font(FONT_SEMIBOLD)
            .style(|t: &Theme| iced::widget::text::Style {
                color: Some(tokens(t).text_secondary),
            }),
    ]
    .spacing(theme::SP_SM)
    .align_y(Alignment::Center);
    if report.blocked {
        stages = stages.push(
            text(report.error.clone().unwrap_or_default())
                .size(11)
                .style(|t: &Theme| iced::widget::text::Style {
                    color: Some(tokens(t).danger),
                }),
        );
        return stages.into();
    }
    for (index, stage) in report.stages.iter().enumerate() {
        if index > 0 {
            stages = stages.push(
                text("→")
                    .size(11)
                    .style(|t: &Theme| iced::widget::text::Style {
                        color: Some(tokens(t).text_tertiary),
                    }),
            );
        }
        let stage_label = if stage.applied {
            format!(
                "{} {}",
                lang.tr(stage.label_key),
                format_line_count(&lang, stage.line_count)
            )
        } else {
            format!(
                "{} ({})",
                lang.tr(stage.label_key),
                lang.tr("mixin_studio_cascade_undeclared")
            )
        };
        stages = stages.push(
            text(stage_label)
                .size(11)
                .font(FONT_MEDIUM)
                .style(|t: &Theme| iced::widget::text::Style {
                    color: Some(tokens(t).text_secondary),
                }),
        );
    }
    iced::widget::column![
        stages,
        text(format!(
            "{}: {}",
            lang.tr("mixin_studio_cascade_merged"),
            format_line_count(&lang, report.merged_line_count())
        ))
        .size(11)
        .style(|t: &Theme| iced::widget::text::Style {
            color: Some(tokens(t).text_tertiary),
        }),
    ]
    .spacing(theme::SP_XS)
    .into()
}

fn format_line_count(lang: &Lang<'_>, count: usize) -> String {
    format!("{} {}", count, lang.tr("mixin_studio_cascade_lines"))
}

/// One column caption: the shared label plus its real line count and whether
/// the column is editable.
fn column_caption<'a>(lang: &Lang<'_>, column: &MixinColumn) -> Element<'a, Message> {
    let mode_key = if column.editable {
        "mixin_column_editable"
    } else {
        "mixin_column_readonly"
    };
    row![
        text(lang.tr(column.label_key).to_string())
            .size(11)
            .font(FONT_SEMIBOLD)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_primary),
            }),
        iced::widget::Space::new().width(theme::SP_XS),
        text(format!(
            "{} · {}",
            format_line_count(lang, column.line_count),
            lang.tr(mode_key)
        ))
        .size(10)
        .font(MONO)
        .style(|t: &Theme| text::Style {
            color: Some(tokens(t).text_tertiary),
        }),
    ]
    .align_y(Alignment::Center)
    .into()
}

/// A read-only document box at a fixed height, aligned with the editor column.
fn read_only_box<'a>(body: String, height_px: f32, danger: bool) -> Element<'a, Message> {
    container(
        crate::view::components::modern_scrollable(text(body).size(11).font(MONO).style(
            move |t: &Theme| text::Style {
                color: Some(if danger {
                    tokens(t).danger
                } else {
                    tokens(t).text_primary
                }),
            },
        ))
        .height(Length::Fixed(height_px)),
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
    })
    .into()
}

/// DUAL-10-09: the three-column Mixin editor — Base document on the left, the
/// editable Mixin overlay in the middle, and the composed document on the
/// right. The composed column is the real output of the shared cascade
/// reduction (or the real blocking reason), never a mock.
pub fn three_column_row(state: &AppState) -> Element<'_, Message> {
    let lang = Lang(&state.shell.lang);
    let base = state.editor.editor_content.text();
    let columns = mixin_studio::mixin_editor_columns(&base, &state.editor.mixin_content.text());

    // The middle column is the real editor; the two read-only columns mirror
    // its exact pixel height so the row reads as one workspace.
    let viewport = editor_viewport::mixin_viewport_for(
        &state.editor.mixin_content,
        state.shell.viewport.height_px,
        state.editor.mixin_viewport.first_line(),
    );
    let body_height = editor_viewport::editor_box_height_px(viewport.rendered_len());

    let base_column = iced::widget::column![
        column_caption(&lang, &columns.base),
        iced::widget::Space::new().height(theme::SP_XS),
        read_only_box(columns.base.content.clone(), body_height, false),
    ]
    .width(Length::FillPortion(1));

    let overlay_column = iced::widget::column![
        column_caption(&lang, &columns.overlay),
        iced::widget::Space::new().height(theme::SP_XS),
        iced::widget::row![
            editor_viewport::gutter(&state.editor.mixin_content, viewport),
            iced::widget::Space::new().width(theme::SP_XS),
            editor_viewport::editor_element(
                &state.editor.mixin_content,
                Message::MixinEditorAction,
                viewport.rendered_len(),
            ),
        ],
    ]
    .width(Length::FillPortion(1));

    let composed_column = iced::widget::column![
        column_caption(&lang, &columns.composed),
        iced::widget::Space::new().height(theme::SP_XS),
        match columns.error.as_deref() {
            Some(error) => iced::widget::column![
                text(lang.tr("mixin_column_blocked").replace("{error}", error))
                    .size(11)
                    .font(MONO)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).danger),
                    }),
                iced::widget::Space::new().height(theme::SP_XS),
                read_only_box(String::new(), (body_height - 28.0).max(40.0), true),
            ],
            None => iced::widget::column![read_only_box(
                columns.composed.content.clone(),
                body_height,
                false,
            )],
        },
    ]
    .width(Length::FillPortion(1));

    iced::widget::row![
        base_column,
        iced::widget::Space::new().width(theme::SP_SM),
        overlay_column,
        iced::widget::Space::new().width(theme::SP_SM),
        composed_column,
    ]
    .width(Length::Fill)
    .into()
}
