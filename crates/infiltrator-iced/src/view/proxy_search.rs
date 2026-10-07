//! Native rich text replays shared name runs; this renderer performs no search or filtering.
use crate::state::AppState;
use crate::types::message::Message;
use crate::view::theme::{self, tokens};
use iced::widget::{rich_text, span};
use iced::{Element, Renderer, Theme};

pub fn node_name<'a>(state: &'a AppState, name: &'a str, size: u32) -> Element<'a, Message> {
    let palette = tokens(&state.shell.theme);
    let runs = state.runtime.proxy_name_runs.get(name);
    let spans: Vec<_> = if let Some(runs) = runs {
        runs.iter()
            .map(|run| {
                span(run.text.as_str()).color(if run.highlighted {
                    palette.accent
                } else {
                    palette.text_primary
                })
            })
            .collect()
    } else {
        vec![span(name).color(palette.text_primary)]
    };
    rich_text::<(), Message, Theme, Renderer>(spans)
        .size(size)
        .font(theme::FONT_SEMIBOLD)
        .into()
}
