//! Native Iced text runs replay the shared connection search decisions.
use crate::state::AppState;
use crate::types::message::Message;
use crate::view::theme::{FONT_SEMIBOLD, tokens};
use iced::advanced::text::Wrapping;
use iced::advanced::widget::Id;
use iced::widget::{rich_text, span};
use iced::{Element, Length, Renderer, Theme};
use infiltrator_contract::search_text::SearchTextRun;

pub(crate) fn highlighted<'a>(
    state: &'a AppState,
    runs: &'a [SearchTextRun],
    size: u32,
) -> Element<'a, Message> {
    let palette = tokens(&state.shell.theme);
    let spans: Vec<_> = runs
        .iter()
        .map(|run| {
            span(run.text.as_str()).color(if run.highlighted {
                palette.accent
            } else {
                palette.text_primary
            })
        })
        .collect();
    rich_text::<(), Message, Theme, Renderer>(spans)
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
