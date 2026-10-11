//! Native Iced text runs replay the shared connection search decisions.
use crate::state::AppState;
use crate::types::message::Message;
use crate::view::theme::{FONT_SEMIBOLD, tokens};
use iced::advanced::text::Wrapping;
use iced::advanced::widget::Id;

use iced::{Element, Length};
use zhuge_ui_core::text::SearchTextRun;
use zhuge_ui_kit_iced::text::highlighted;

pub(crate) fn connection_text<'a>(
    state: &'a AppState,
    runs: &'a [SearchTextRun],
    size: u32,
) -> Element<'a, Message> {
    let palette = tokens(&state.shell.theme);
    highlighted(
        runs.iter().map(|run| (run.text.as_str(), run.highlighted)),
        palette.text_primary,
        palette.accent,
    )
    .size(size)
    .font(FONT_SEMIBOLD)
    .width(Length::Fill)
    .wrapping(Wrapping::WordOrGlyph)
    .into()
}

pub(crate) fn row_id(id: &str) -> Id {
    Id::from(format!("connection-row-{id}"))
}

pub(crate) fn highlight_id(id: &str) -> Id {
    Id::from(format!("connection-highlight-{id}"))
}
