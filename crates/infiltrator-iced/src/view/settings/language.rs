//! Native language controls render the neutral durable-save state.
use crate::state::AppState;
use crate::types::message::Message;
use crate::view::component_card::card;
use crate::view::component_forms::{style_accent, style_ghost};
use crate::view_root::interaction_regions::InteractionRegion;
use iced::widget::{button, column, container, row, text};
use iced::{Element, Length};
use infiltrator_contract::language::LanguagePreference;
use infiltrator_shared::locales::{Lang, Localizer};

pub fn language_card(state: &AppState) -> Element<'_, Message> {
    let lang = Lang(&state.shell.lang);
    container(card(
        Some(lang.tr("settings_language_title").into_owned()),
        language_controls(state),
    ))
    .id(InteractionRegion::LanguageChoices.id())
    .into()
}
pub fn language_controls(state: &AppState) -> Element<'_, Message> {
    let lang = Lang(&state.shell.lang);
    let model = &state.shell.language_choice;
    let mut choices = row![].spacing(8);
    for preference in LanguagePreference::ALL {
        let active = model.applied == Some(preference);
        choices = choices.push(
            button(text(lang.tr(preference.label_key())))
                .style(if active { style_accent } else { style_ghost })
                .on_press_maybe(
                    (!active && model.pending.is_none() && (model.can_persist || state.shell.demo))
                        .then(|| Message::SetLanguage(preference.as_setting().into())),
                ),
        );
    }
    let status = model
        .failure
        .as_ref()
        .map(|failure| failure.message.clone())
        .unwrap_or_else(|| {
            if model.pending.is_some() {
                lang.tr("language_saving").into_owned()
            } else if !model.can_persist && !state.shell.demo {
                lang.tr("language_save_unavailable").into_owned()
            } else {
                String::new()
            }
        });
    let retry = button(text(lang.tr("language_retry")))
        .style(style_ghost)
        .on_press_maybe(
            (model.failure.is_some() && model.pending.is_none() && model.can_persist)
                .then_some(Message::RetryLanguageChoice),
        );
    column![
        choices,
        row![text(status).size(12).width(Length::Fill), retry].spacing(8)
    ]
    .spacing(8)
    .into()
}
