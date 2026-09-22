//! DUAL-09-02/13: the Iced editor viewport — fixed line metrics, the shared
//! bounded window and the line-number gutter.
//!
//! The Iced `text_editor` widget owns its own pixel scroll offset and exposes
//! no getter, so the surface keeps the two facts it needs:
//!
//! * the editor is laid out at an exact multiple of [`EDITOR_LINE_HEIGHT_PX`],
//!   which makes the widget's internal viewport an exact number of lines;
//! * the shared [`EditorViewport`] mirrors the widget's own published
//!   `Action::Scroll` deltas and applies the shared clamp.
//!
//! Because both use the same line count and the same window arithmetic, the
//! gutter renders exactly the lines the widget is showing. The window is
//! *bounded*: the widget renders `window_lines` lines and the gutter renders
//! `window_lines` numbers, never the whole document (DUAL-09-13). No
//! frame-rate claim is made.

use crate::types::message::Message;
use crate::view::theme::{self, MONO, tokens};
use iced::advanced::text::LineHeight;
use iced::widget::text_editor;
use iced::widget::{Row, Space, column, container, row, text};
use iced::{Alignment, Element, Length, Pixels, Theme};
use infiltrator_contract::editor_viewport::{EditorViewport, line_indent_level};
use infiltrator_shared::locales::{Lang, Localizer};

/// Line height the editor and its gutter share, in pixels. Both the editor's
/// `line_height` and every gutter row use this exact value, so the two columns
/// stay aligned line for line.
pub const EDITOR_LINE_HEIGHT_PX: f32 = 18.0;

/// Inner padding of the editor box; the gutter repeats it as its top inset.
pub const EDITOR_PADDING_PX: f32 = 14.0;

/// Pixels the profile pane spends above/below the editor box (page padding,
/// toolbar, snippet bar, banners). The window is sized against this estimate.
pub const EDITOR_CHROME_PX: f32 = 300.0;

/// Smallest window the surface will show, even on a short window.
pub const EDITOR_MIN_WINDOW_LINES: usize = 8;

/// Largest window the surface will show; the rest of a tall window stays part
/// of the card instead of stretching the editor across the screen.
pub const EDITOR_MAX_WINDOW_LINES: usize = 28;

/// Gutter column width (numbers + indentation rail).
const GUTTER_WIDTH_PX: f32 = 52.0;

/// One indentation rail step, in pixels.
const INDENT_RAIL_STEP_PX: f32 = 4.0;

/// How many lines the editor window shows for a given window height.
pub fn window_lines_for_window_height(height_px: f32) -> usize {
    let chrome = EDITOR_CHROME_PX + 2.0 * EDITOR_PADDING_PX;
    let usable = (height_px - chrome).max(EDITOR_LINE_HEIGHT_PX * EDITOR_MIN_WINDOW_LINES as f32);
    ((usable / EDITOR_LINE_HEIGHT_PX).floor() as usize)
        .clamp(EDITOR_MIN_WINDOW_LINES, EDITOR_MAX_WINDOW_LINES)
}

/// DUAL-10-09: chrome the Mixin pane spends above/below its three-column
/// workspace — the preflight banner, the toggle chips, the cascade strip, the
/// export panel and the history row. The Mixin window is sized against this
/// larger estimate so the workspace and the export panel stay inside the
/// (non-scrolling) page on a normal window.
pub const MIXIN_EDITOR_CHROME_PX: f32 = 470.0;

/// The bounded window for the Mixin pane's middle column.
pub fn mixin_viewport_for(
    content: &text_editor::Content,
    window_height_px: f32,
    first_line: usize,
) -> EditorViewport {
    let chrome = MIXIN_EDITOR_CHROME_PX + 2.0 * EDITOR_PADDING_PX;
    let usable =
        (window_height_px - chrome).max(EDITOR_LINE_HEIGHT_PX * EDITOR_MIN_WINDOW_LINES as f32);
    let window_lines = ((usable / EDITOR_LINE_HEIGHT_PX).floor() as usize)
        .clamp(EDITOR_MIN_WINDOW_LINES, EDITOR_MAX_WINDOW_LINES);
    EditorViewport::new(content.line_count().max(1), window_lines, first_line)
}

/// Exact pixel height of the editor box for a window of `window_lines` lines.
pub fn editor_box_height_px(window_lines: usize) -> f32 {
    window_lines as f32 * EDITOR_LINE_HEIGHT_PX + 2.0 * EDITOR_PADDING_PX
}

/// The window for the current content and pane height.
pub fn viewport_for(
    content: &text_editor::Content,
    window_height_px: f32,
    first_line: usize,
) -> EditorViewport {
    EditorViewport::new(
        content.line_count().max(1),
        window_lines_for_window_height(window_height_px),
        first_line,
    )
}

/// The windowed editor: exactly `window_lines` visible lines, no wrapping, so
/// one document line is one rendered line and the gutter cannot drift.
pub fn editor_element<'a>(
    content: &'a text_editor::Content,
    on_action: impl Fn(text_editor::Action) -> Message + 'a,
    window_lines: usize,
) -> Element<'a, Message> {
    text_editor(content)
        .on_action(on_action)
        .font(MONO)
        .size(12.0)
        .line_height(LineHeight::Absolute(Pixels(EDITOR_LINE_HEIGHT_PX)))
        .padding(EDITOR_PADDING_PX as u16)
        .wrapping(iced::advanced::text::Wrapping::None)
        .height(Length::Fixed(editor_box_height_px(window_lines)))
        .into()
}

/// One gutter row: the document line number plus the shared indentation rail.
fn gutter_row<'a>(line: usize, indent_level: usize) -> Element<'a, Message> {
    let number_style = |t: &Theme| iced::widget::text::Style {
        color: Some(tokens(t).text_tertiary),
    };
    let rail: Vec<Element<'a, Message>> = (0..indent_level)
        .map(|_| {
            container(Space::new())
                .width(Length::Fixed(2.0))
                .height(Length::Fixed(INDENT_RAIL_STEP_PX * 2.0))
                .style(|t: &Theme| container::Style {
                    background: Some(tokens(t).accent_soft.into()),
                    ..Default::default()
                })
                .into()
        })
        .collect();
    container(
        row![
            container(
                text(format!("{line:>3}"))
                    .size(10)
                    .font(MONO)
                    .line_height(LineHeight::Absolute(Pixels(EDITOR_LINE_HEIGHT_PX)))
                    .style(number_style),
            )
            .width(Length::Fixed(30.0))
            .align_x(Alignment::End),
            Row::with_children(rail).spacing(2.0),
        ]
        .align_y(Alignment::Center),
    )
    .height(Length::Fixed(EDITOR_LINE_HEIGHT_PX))
    .align_y(Alignment::Center)
    .into()
}

/// The line-number gutter for the window the editor is showing.
///
/// Only the window's lines are looked up and rendered: a 10,000-line document
/// still builds `window_lines` rows.
pub fn gutter<'a>(
    content: &text_editor::Content,
    viewport: EditorViewport,
) -> Element<'a, Message> {
    let mut rows: Vec<Element<'a, Message>> = Vec::with_capacity(viewport.rendered_len());
    for line in viewport.line_numbers() {
        let indent = content
            .line(line - 1)
            .map(|content_line| line_indent_level(&content_line.text))
            .unwrap_or(0);
        rows.push(gutter_row(line, indent));
    }
    container(column(rows).spacing(0.0))
        .width(Length::Fixed(GUTTER_WIDTH_PX))
        .height(Length::Fixed(editor_box_height_px(viewport.rendered_len())))
        .padding(iced::Padding {
            top: EDITOR_PADDING_PX,
            right: 6.0,
            bottom: 0.0,
            left: 0.0,
        })
        .style(|t: &Theme| {
            let tk = tokens(t);
            container::Style {
                background: Some(tk.chip_bg.into()),
                border: iced::Border {
                    radius: iced::border::Radius::from(theme::R_CONTROL),
                    ..Default::default()
                },
                ..Default::default()
            }
        })
        .into()
}

/// The shared viewport readout both surfaces show when a document is long
/// enough to be windowed.
pub fn viewport_label(
    viewport: EditorViewport,
    lang: &Lang<'_>,
) -> Option<Element<'static, Message>> {
    if viewport.covers_document() {
        return None;
    }
    let label = format!(
        "{} {} · {}",
        lang.tr("editor_viewport_label"),
        viewport.range_label(),
        window_note(viewport, lang)
    );
    let style = |t: &Theme| iced::widget::text::Style {
        color: Some(tokens(t).text_tertiary),
    };
    Some(
        container(text(label).size(10).font(MONO).style(style))
            .padding([3, 8])
            .style(|t: &Theme| {
                let tk = tokens(t);
                container::Style {
                    background: Some(tk.chip_bg.into()),
                    border: iced::Border {
                        radius: iced::border::Radius::from(theme::R_CHIP),
                        ..Default::default()
                    },
                    ..Default::default()
                }
            })
            .into(),
    )
}

/// Explicit count of the lines this window does not render, in the surface's
/// own locale (no hardcoded CJK literal survives in the view tree).
fn window_note(viewport: EditorViewport, lang: &Lang<'_>) -> String {
    use infiltrator_shared::i18n_interpolator::interpolate;

    let above = (viewport.hidden_above() > 0).then(|| {
        interpolate(
            &lang.tr("editor_viewport_hidden_above"),
            &[("count", &viewport.hidden_above().to_string())],
        )
    });
    let below = (viewport.hidden_below() > 0).then(|| {
        interpolate(
            &lang.tr("editor_viewport_hidden_below"),
            &[("count", &viewport.hidden_below().to_string())],
        )
    });
    match (above, below) {
        (Some(above), Some(below)) => format!("{above} · {below}"),
        (Some(above), None) => above,
        (None, Some(below)) => below,
        (None, None) => String::new(),
    }
}

#[cfg(test)]
#[path = "../../tests/gui/view_editor_viewport_tests.rs"]
mod tests;
