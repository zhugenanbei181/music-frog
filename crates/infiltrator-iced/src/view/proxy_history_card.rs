//! The history entry opens the shared node inspector; measurements never originate in UI data.
use crate::state::AppState;
use crate::types::message::Message;
use crate::view::component_card::card;
use crate::view::component_forms::style_accent;
use iced::Element;
use iced::widget::{button, column, text};
use infiltrator_shared::locales::{Lang, Localizer};
pub fn proxy_history_card<'a>(state: &'a AppState, lang: &Lang<'_>) -> Element<'a, Message> {
    let member = state
        .runtime
        .active_exit
        .name
        .as_ref()
        .filter(|_| state.runtime.active_exit.is_drawable())
        .and_then(|name| {
            state
                .runtime
                .proxy_groups
                .iter()
                .flat_map(|group| &group.proxies)
                .find(|member| member.search_name() == name || member.name == *name)
        });
    let name = member
        .map(|member| member.search_name().to_owned())
        .unwrap_or_else(|| lang.tr("latency_select_node").into_owned());
    let message = member
        .and_then(|member| member.id.clone())
        .map(|id| Message::InspectOutbound(Some(id)));
    card(
        Some(lang.tr("proxy_inspection_history").into_owned()),
        column![
            text(name).size(12),
            button(text(lang.tr("proxy_inspection_title")))
                .style(style_accent)
                .on_press_maybe(message)
        ]
        .spacing(8),
    )
}
