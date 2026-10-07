//! Settings reads the actual shared apply receipt and opens the existing profile editor.
use crate::state::AppState;
use crate::types::app::Route;
use crate::types::message::Message;
use crate::view::component_card::card;
use crate::view::component_forms::style_accent;
use crate::view::theme::FONT_MEDIUM;
use iced::Element;
use iced::widget::{button, column, text};
use infiltrator_application::profile_editor_projection::transaction;
use infiltrator_shared::locales::{Lang, Localizer};
pub fn apply_guard_card<'a>(state: &'a AppState, lang: &Lang<'_>) -> Element<'a, Message> {
    let status = state
        .editor
        .apply_transaction
        .as_ref()
        .map(|record| transaction(record, lang.0))
        .unwrap_or_else(|| lang.tr("editor_apply_idle").into_owned());
    card(
        Some(lang.tr("apply_guard_title").into_owned()),
        column![
            text(status).size(12),
            button(
                text(lang.tr("configuration_open_editor_action"))
                    .size(11)
                    .font(FONT_MEDIUM)
            )
            .style(style_accent)
            .on_press(Message::Navigate(Route::Editor))
        ]
        .spacing(8),
    )
}
