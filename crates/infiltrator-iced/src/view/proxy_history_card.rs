//! The history entry opens the shared node inspector; measurements never originate in UI data.
use crate::state::AppState;
use crate::types::message::Message;
use crate::view::component_card::card;
use crate::view::component_forms::style_accent;
use iced::Element;
use iced::widget::{button, column, text};
use infiltrator_shared::locales::{Lang, Localizer};
pub fn proxy_history_card<'a>(state: &'a AppState, lang: &Lang<'_>) -> Element<'a, Message> {
    let node = state.runtime.active_exit.name.as_ref().filter(|name| {
        state.runtime.active_exit.is_drawable() && state.proxy_inspection(name).is_some()
    });
    let name = node
        .cloned()
        .unwrap_or_else(|| lang.tr("latency_select_node").into_owned());
    card(
        Some(lang.tr("proxy_inspection_history").into_owned()),
        column![
            text(name).size(12),
            button(text(lang.tr("proxy_inspection_title")))
                .style(style_accent)
                .on_press_maybe(node.map(|name| Message::InspectProxy(Some(name.clone()))))
        ]
        .spacing(8),
    )
}
