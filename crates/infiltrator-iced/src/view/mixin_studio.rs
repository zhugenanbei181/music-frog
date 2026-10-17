//! DUAL-10-08/10/11: the Iced Mixin pane's shared-studio widgets.
//!
//! The pane renders engine-scoped codec projections and routes edits through
//! the typed Core application seam.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::components::modern_scrollable;
use crate::view::theme::{FONT_MEDIUM, FONT_SEMIBOLD, MONO, tokens};
use crate::view::{editor_viewport, theme};
use iced::widget::{Space, button, column, container, row, text};
use iced::{Alignment, Border, Color, Element, Length, Theme, border};
use infiltrator_contract::profile_mixin_studio::{
    ProfileMixinCascade, ProfileMixinColumn, ProfileMixinColumnRole, ProfileMixinComposition,
    ProfileMixinPreflight, ProfileMixinStudioObservation, ProfileMixinToggleId,
};
use infiltrator_presentation::mixin_studio_projection;
use infiltrator_shared::locales::{Lang, Localizer};
use lurker_core_contract::error::{ErrorCode, Failure};

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

fn inspect_studio(state: &AppState) -> Result<ProfileMixinStudioObservation, Failure> {
    let identity = state
        .editor
        .document_session
        .source()
        .map(|source| source.document.clone())
        .ok_or_else(|| Failure::new(ErrorCode::NotReady, "No profile document is open", true))?;
    let commands = state.command_ingress().ok_or_else(|| {
        Failure::new(
            ErrorCode::NotReady,
            "Profile options codec is unavailable",
            true,
        )
    })?;
    commands.inspect_mixin_studio(
        &identity,
        &state.editor.editor_content.text(),
        &state.editor.mixin_content.text(),
    )
}

/// DUAL-10-11: the common-overlay toggle chips, state read through the shared
/// catalogue and the real mixin codec.
pub fn toggle_row(state: &AppState) -> Element<'_, Message> {
    let lang = Lang(&state.shell.lang);
    let mixin = state.editor.mixin_content.text();
    let identity = state
        .editor
        .document_session
        .source()
        .map(|source| source.document.clone());
    let commands = state.command_ingress();
    let mut chips = row![
        text(lang.tr("mixin_studio_toggles_title").to_string())
            .size(11)
            .font(FONT_SEMIBOLD)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary),
            }),
    ]
    .spacing(theme::SP_SM)
    .align_y(Alignment::Center);
    for toggle in ProfileMixinToggleId::ALL {
        let enabled = identity
            .as_ref()
            .zip(commands.as_ref())
            .and_then(|(identity, commands)| {
                commands.inspect_mixin_toggle(identity, &mixin, toggle).ok()
            })
            .unwrap_or(false);
        let button_label = lang
            .tr(mixin_studio_projection::preset_key(toggle))
            .to_string();
        chips = chips.push(
            button(text(button_label).size(11).font(FONT_MEDIUM))
                .padding([4, 10])
                .style(chip_style(enabled))
                .on_press(Message::ToggleMixinPreset(toggle, !enabled)),
        );
    }
    chips.into()
}

/// DUAL-10-10: the engine-scoped codec verdict for the current overlay. A
/// blocking verdict states the real reason the save is refused.
pub fn preflight_banner(state: &AppState) -> Element<'_, Message> {
    let lang = Lang(&state.shell.lang);
    let (accepted, detail) = match inspect_studio(state) {
        Ok(projection) => {
            let accepted = matches!(&projection.preflight, ProfileMixinPreflight::Accepted);
            (
                accepted,
                mixin_studio_projection::preflight_detail(&projection.preflight, lang.0),
            )
        }
        Err(failure) => (false, failure.message),
    };
    let danger = !accepted;
    let headline = if accepted {
        lang.tr("mixin_studio_preflight_ok").to_string()
    } else {
        lang.tr("mixin_studio_preflight_blocked").to_string()
    };
    container(
        column![
            text(headline)
                .size(11)
                .font(FONT_SEMIBOLD)
                .style(move |t: &Theme| text::Style {
                    color: Some(if danger {
                        tokens(t).danger
                    } else {
                        tokens(t).success
                    })
                }),
            text(detail)
                .size(11)
                .font(FONT_MEDIUM)
                .style(|t: &Theme| text::Style {
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

    let mut stages = row![
        text(lang.tr("mixin_studio_cascade_title").to_string())
            .size(11)
            .font(FONT_SEMIBOLD)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary),
            }),
    ]
    .spacing(theme::SP_SM)
    .align_y(Alignment::Center);
    let projection = match inspect_studio(state) {
        Ok(projection) => projection,
        Err(failure) => {
            return stages
                .push(
                    text(failure.message)
                        .size(11)
                        .style(|t: &Theme| text::Style {
                            color: Some(tokens(t).danger),
                        }),
                )
                .into();
        }
    };
    let report = projection.cascade;
    if let ProfileMixinCascade::Blocked { reason, .. } = &report {
        stages = stages.push(
            text(reason.clone())
                .size(11)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).danger),
                }),
        );
        return stages.into();
    }
    let ProfileMixinCascade::Merged {
        stages: report_stages,
        line_count,
    } = &report
    else {
        unreachable!("blocked cascade returned above")
    };
    for (index, stage) in report_stages.iter().enumerate() {
        if index > 0 {
            stages = stages.push(text("→").size(11).style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_tertiary),
            }));
        }
        let stage_label = mixin_studio_projection::stage_caption(stage, lang.0);
        stages = stages.push(
            text(stage_label)
                .size(11)
                .font(FONT_MEDIUM)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary),
                }),
        );
    }
    column![
        stages,
        text(format!(
            "{}: {}",
            lang.tr("mixin_studio_cascade_merged"),
            format_line_count(&lang, *line_count)
        ))
        .size(11)
        .style(|t: &Theme| text::Style {
            color: Some(tokens(t).text_tertiary),
        }),
    ]
    .spacing(theme::SP_XS)
    .into()
}

fn format_line_count(lang: &Lang<'_>, count: usize) -> String {
    mixin_studio_projection::line_count(count, lang.0)
}

/// One column caption: the shared label plus its real line count and whether
/// the column is editable.
fn column_caption<'a>(lang: &Lang<'_>, column: &ProfileMixinColumn) -> Element<'a, Message> {
    text(mixin_studio_projection::column_caption(column, lang.0))
        .size(11)
        .font(FONT_SEMIBOLD)
        .style(|t: &Theme| text::Style {
            color: Some(tokens(t).text_primary),
        })
        .into()
}

/// A read-only document box at a fixed height, aligned with the editor column.
fn read_only_box<'a>(body: String, height_px: f32, danger: bool) -> Element<'a, Message> {
    container(
        modern_scrollable(
            text(body)
                .size(11)
                .font(MONO)
                .style(move |t: &Theme| text::Style {
                    color: Some(if danger {
                        tokens(t).danger
                    } else {
                        tokens(t).text_primary
                    }),
                }),
        )
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
/// editable Mixin overlay in the middle, and the typed codec observation on the
/// right, including its blocking reason when composition is refused.
pub fn three_column_row(state: &AppState) -> Element<'_, Message> {
    let lang = Lang(&state.shell.lang);
    let base = state.editor.editor_content.text();
    let overlay = state.editor.mixin_content.text();
    let result = inspect_studio(state);
    let fallback_base = ProfileMixinColumn {
        role: ProfileMixinColumnRole::Base,
        line_count: base.lines().count(),
        content: base.clone(),
        editable: false,
    };
    let fallback_overlay = ProfileMixinColumn {
        role: ProfileMixinColumnRole::Overlay,
        line_count: overlay.lines().count(),
        content: overlay.clone(),
        editable: true,
    };
    let (base_column_model, overlay_column_model, composed, compose_error) = match result {
        Ok(projection) => {
            let composition = projection.columns.composed;
            let (composed, compose_error) = match composition {
                ProfileMixinComposition::Produced(column) => (column, None),
                ProfileMixinComposition::Blocked { reason } => (
                    ProfileMixinColumn {
                        role: ProfileMixinColumnRole::Composed,
                        content: String::new(),
                        editable: false,
                        line_count: 0,
                    },
                    Some(reason),
                ),
            };
            (
                projection.columns.base,
                projection.columns.overlay,
                composed,
                compose_error,
            )
        }
        Err(failure) => (
            fallback_base,
            fallback_overlay,
            ProfileMixinColumn {
                role: ProfileMixinColumnRole::Composed,
                content: String::new(),
                editable: false,
                line_count: 0,
            },
            Some(failure.message),
        ),
    };

    // The middle column is the real editor; the two read-only columns mirror
    // its exact pixel height so the row reads as one workspace.
    let viewport = editor_viewport::mixin_viewport_for(
        &state.editor.mixin_content,
        state.shell.viewport.height_px,
        state.editor.mixin_viewport.first_line(),
    );
    let body_height = editor_viewport::editor_box_height_px(viewport.rendered_len());

    let base_column = column![
        column_caption(&lang, &base_column_model),
        Space::new().height(theme::SP_XS),
        read_only_box(base_column_model.content.clone(), body_height, false),
    ]
    .width(Length::FillPortion(1));

    let overlay_column = column![
        column_caption(&lang, &overlay_column_model),
        Space::new().height(theme::SP_XS),
        row![
            editor_viewport::gutter(&state.editor.mixin_content, viewport),
            Space::new().width(theme::SP_XS),
            editor_viewport::editor_element(
                &state.editor.mixin_content,
                Message::MixinEditorAction,
                viewport.rendered_len(),
                state.editor.mixin_session.can_edit(),
            ),
        ],
    ]
    .width(Length::FillPortion(1));

    let composed_column = column![
        column_caption(&lang, &composed),
        Space::new().height(theme::SP_XS),
        match compose_error.as_deref() {
            Some(error) => column![
                text(lang.tr("mixin_column_blocked").replace("{error}", error))
                    .size(11)
                    .font(MONO)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).danger),
                    }),
                Space::new().height(theme::SP_XS),
                read_only_box(String::new(), (body_height - 28.0).max(40.0), true),
            ],
            None => column![read_only_box(composed.content.clone(), body_height, false,)],
        },
    ]
    .width(Length::FillPortion(1));

    row![
        base_column,
        Space::new().width(theme::SP_SM),
        overlay_column,
        Space::new().width(theme::SP_SM),
        composed_column,
    ]
    .width(Length::Fill)
    .into()
}
