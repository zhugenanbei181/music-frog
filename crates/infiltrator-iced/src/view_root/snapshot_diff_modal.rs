//! Snapshot Visual Diff & Rollback Modal Dialog (DUAL-09-08 / DUAL-09-09).
//!
//! The modal renders the shared `YamlAstDiffSnapshot` the snapshot application
//! computed from the real snapshot file and the current profile content: a
//! unified (inline) or side-by-side (split) layout, real `+/-/~` colors and
//! line numbers, and a two-step confirmed rollback through the apply
//! transaction.

use crate::state::AppState;
use crate::types::app::SnapshotDiffMode;
use crate::types::message::Message;
use crate::view::components::{
    BadgeKind, badge, icon_button, modern_scrollable, segmented_control, style_accent, style_ghost,
};
use crate::view::svg_icons::{self, Icon};
use crate::view::theme::{self, FONT_MEDIUM, FONT_SEMIBOLD, MONO, tokens};
use iced::widget::{Space, button, column, container, row, text};
use iced::{Alignment, Border, Color, Element, Length, Theme, border};
use infiltrator_contract::yaml_ast_diff::{DiffKind, DiffLine, SplitDiffRow, YamlAstDiffSnapshot};
use infiltrator_shared::locales::{Lang, Localizer};

pub fn snapshot_diff_modal<'a>(state: &'a AppState, snapshot_id: &str) -> Element<'a, Message> {
    let lang = Lang(&state.shell.lang);

    let title_row = row![
        svg_icons::icon_themed(Icon::FileText, 18.0, |t: &Theme| tokens(t).accent),
        Space::new().width(theme::SP_SM),
        text(lang.tr("snapshot_diff_title").to_string())
            .size(15)
            .font(FONT_SEMIBOLD)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_primary),
            }),
        Space::new().width(theme::SP_SM),
        badge(snapshot_id.to_string(), BadgeKind::Neutral),
        Space::new().width(Length::Fill),
        icon_button(Icon::X, 14.0, Message::CloseSnapshotDiff),
    ]
    .align_y(Alignment::Center);

    let subtitle = text(format!(
        "{}: {snapshot_id}",
        lang.tr("snapshot_diff_compare_with"),
    ))
    .size(12)
    .style(|t: &Theme| text::Style {
        color: Some(tokens(t).text_secondary),
    });

    let body = match &state.editor.snapshot_diff {
        _ if state.editor.snapshot_diff_loading => {
            message_block(lang.tr("snapshot_diff_loading").to_string(), |t: &Theme| {
                tokens(t).text_tertiary
            })
        }
        _ if state.editor.snapshot_diff_error.is_some() => message_block(
            state.editor.snapshot_diff_error.clone().unwrap_or_default(),
            |t: &Theme| tokens(t).danger,
        ),
        Some(diff) if diff.is_empty() => {
            message_block(lang.tr("snapshot_diff_empty").to_string(), |t: &Theme| {
                tokens(t).text_tertiary
            })
        }
        Some(diff) => diff_body(diff, state.editor.snapshot_diff_mode),
        None => message_block(lang.tr("snapshot_diff_empty").to_string(), |t: &Theme| {
            tokens(t).text_tertiary
        }),
    };

    let stats_row = state.editor.snapshot_diff.as_ref().map(|diff| {
        row![
            badge(diff.change_summary(), BadgeKind::Neutral),
            Space::new().width(theme::SP_SM),
            badge(
                lang.tr("snapshot_diff_fidelity").to_string(),
                if diff.fidelity_preserved {
                    BadgeKind::Success
                } else {
                    BadgeKind::Warning
                },
            ),
            Space::new().width(theme::SP_SM),
            text(diff.fidelity_grade.as_str().to_string())
                .size(11)
                .font(MONO)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary),
                }),
        ]
        .align_y(Alignment::Center)
    });

    let mode_labels = vec![
        lang.tr("snapshot_diff_inline").to_string(),
        lang.tr("snapshot_diff_split").to_string(),
    ];
    let mode_index = match state.editor.snapshot_diff_mode {
        SnapshotDiffMode::Inline => 0,
        SnapshotDiffMode::Split => 1,
    };
    let mode_switch = segmented_control(&mode_labels, mode_index, |index| {
        Message::SetSnapshotDiffMode(if index == 1 {
            SnapshotDiffMode::Split
        } else {
            SnapshotDiffMode::Inline
        })
    });

    let diff_container = container(modern_scrollable(body).height(Length::Fixed(260.0)))
        .padding([12, 16])
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
        });

    let target_id = snapshot_id.to_string();
    let rollback_controls: Element<'a, Message> = if state.editor.snapshot_diff_rollback_armed {
        row![
            text(lang.tr("snapshot_diff_confirm_hint").to_string())
                .size(11)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).warning),
                }),
            Space::new().width(theme::SP_SM),
            button(text(lang.tr("btn_cancel").to_string()).size(12))
                .padding([6, 14])
                .style(style_ghost)
                .on_press(Message::CancelSnapshotRollback),
            Space::new().width(theme::SP_SM),
            button(
                text(lang.tr("snapshot_diff_confirm_btn").to_string())
                    .size(12)
                    .font(FONT_MEDIUM)
            )
            .padding([6, 16])
            .style(style_accent)
            .on_press(Message::RollbackToSnapshot(target_id)),
        ]
        .align_y(Alignment::Center)
        .into()
    } else {
        button(
            row![
                svg_icons::icon_themed(Icon::RefreshCw, 12.0, |t: &Theme| tokens(t).on_accent),
                Space::new().width(theme::SP_XS),
                text(lang.tr("snapshot_diff_rollback_btn").to_string())
                    .size(12)
                    .font(FONT_MEDIUM),
            ]
            .align_y(Alignment::Center),
        )
        .padding([6, 16])
        .style(style_accent)
        .on_press(Message::ArmSnapshotRollback)
        .into()
    };

    let actions = row![
        button(text(lang.tr("btn_cancel").to_string()).size(12))
            .padding([6, 14])
            .style(style_ghost)
            .on_press(Message::CloseSnapshotDiff),
        Space::new().width(Length::Fill),
        rollback_controls,
    ]
    .align_y(Alignment::Center);

    let mut card = column![
        title_row,
        subtitle,
        Space::new().height(theme::SP_XS),
        mode_switch,
    ]
    .spacing(theme::SP_SM);
    if let Some(stats) = stats_row {
        card = card.push(stats);
    }
    let card = card
        .push(Space::new().height(theme::SP_XS))
        .push(diff_container)
        .push(Space::new().height(theme::SP_MD))
        .push(actions);

    let modal_card = container(card)
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

fn message_block<'a>(message: String, color: fn(&Theme) -> Color) -> Element<'a, Message> {
    text(message)
        .size(12)
        .style(move |t: &Theme| text::Style {
            color: Some(color(t)),
        })
        .into()
}

fn diff_body<'a>(diff: &'a YamlAstDiffSnapshot, mode: SnapshotDiffMode) -> Element<'a, Message> {
    match mode {
        SnapshotDiffMode::Inline => {
            let mut lines = column![].spacing(theme::SP_XS);
            for line in &diff.unified_lines {
                lines = lines.push(unified_row(line));
            }
            lines.into()
        }
        SnapshotDiffMode::Split => {
            let mut rows = column![].spacing(theme::SP_XS);
            for row_data in &diff.split_rows {
                rows = rows.push(split_row(row_data));
            }
            rows.into()
        }
    }
}

fn line_number_label(line: &DiffLine) -> String {
    match (line.old_line, line.new_line) {
        (Some(old), Some(new)) => format!("{old:>4} {new:>4}"),
        (Some(old), None) => format!("{old:>4}     "),
        (None, Some(new)) => format!("     {new:>4}"),
        (None, None) => "        ".to_string(),
    }
}

fn kind_color(kind: DiffKind) -> Color {
    match kind {
        DiffKind::Insert => Color::from_rgb8(0x3f, 0xb9, 0x50),
        DiffKind::Delete => Color::from_rgb8(0xe5, 0x53, 0x53),
        DiffKind::Modify => Color::from_rgb8(0xd6, 0xa1, 0x2b),
        DiffKind::Equal => Color::from_rgb8(0x8a, 0x8a, 0x8a),
    }
}

fn color_or_secondary(t: &Theme, kind: DiffKind) -> Color {
    match kind {
        DiffKind::Equal => tokens(t).text_secondary,
        _ => kind_color(kind),
    }
}

fn unified_row(line: &DiffLine) -> Element<'_, Message> {
    row![
        text(line_number_label(line))
            .size(10)
            .font(MONO)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_tertiary),
            }),
        Space::new().width(theme::SP_XS),
        text(format!("{}{}", line.kind.symbol(), line.content))
            .size(11)
            .font(MONO)
            .style(move |t: &Theme| text::Style {
                color: Some(color_or_secondary(t, line.kind)),
            }),
    ]
    .align_y(Alignment::Center)
    .into()
}

fn split_side<'a>(line: Option<&'a DiffLine>) -> Element<'a, Message> {
    let Some(line) = line else {
        return text("").into();
    };
    row![
        text(line_number_label(line))
            .size(10)
            .font(MONO)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_tertiary),
            }),
        Space::new().width(theme::SP_XS),
        text(format!("{}{}", line.kind.symbol(), line.content))
            .size(11)
            .font(MONO)
            .style(move |t: &Theme| text::Style {
                color: Some(color_or_secondary(t, line.kind)),
            }),
    ]
    .align_y(Alignment::Center)
    .into()
}

fn split_row(row_data: &SplitDiffRow) -> Element<'_, Message> {
    let left = split_side(row_data.left.as_ref());
    let right = split_side(row_data.right.as_ref());
    row![
        container(left).width(Length::FillPortion(1)),
        Space::new().width(theme::SP_SM),
        container(right).width(Length::FillPortion(1)),
    ]
    .align_y(Alignment::Center)
    .into()
}

#[cfg(test)]
#[path = "../../tests/gui/view_snapshot_diff_tests.rs"]
mod view_snapshot_diff_tests;
