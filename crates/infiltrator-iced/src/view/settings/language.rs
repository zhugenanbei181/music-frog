//! Native language controls render the neutral durable-save state.
use crate::state::AppState;
use crate::types::message::Message;
use crate::view::component_card::card;
use crate::view::component_forms::{style_accent, style_ghost};
use crate::view_root::interaction_regions::InteractionRegion;
use iced::widget::{button, column, container, row, text};
use iced::{Element, Length};
use infiltrator_application::language_choice::{
    LanguageChoicePhase, LanguagePersistence, LanguageReadPhase,
};
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
        let active = match &model.read {
            LanguageReadPhase::Unobserved => false,
            LanguageReadPhase::Observed {
                preference: applied,
            } => *applied == Some(preference),
            LanguageReadPhase::Failed { last_known, .. } => *last_known == Some(preference),
        };
        let busy = match &model.phase {
            LanguageChoicePhase::Pending(_) | LanguageChoicePhase::AwaitingObservation { .. } => {
                true
            }
            LanguageChoicePhase::Ready
            | LanguageChoicePhase::Rejected(_)
            | LanguageChoicePhase::Failed { .. } => false,
        };
        let persistence_available =
            model.persistence == LanguagePersistence::Available || state.shell.demo;
        choices = choices.push(
            button(text(lang.tr(preference.label_key())))
                .style(if active { style_accent } else { style_ghost })
                .on_press_maybe(
                    (!active && !busy && persistence_available)
                        .then(|| Message::SetLanguage(preference.as_setting().into())),
                ),
        );
    }
    let status = match (&model.phase, &model.read, model.persistence) {
        (LanguageChoicePhase::Rejected(failure), _, _)
        | (LanguageChoicePhase::Failed { failure, .. }, _, _) => failure.message.clone(),
        (LanguageChoicePhase::Pending(_), _, _)
        | (LanguageChoicePhase::AwaitingObservation { .. }, _, _) => {
            lang.tr("language_saving").into_owned()
        }
        (LanguageChoicePhase::Ready, LanguageReadPhase::Failed { failure, .. }, _) => {
            failure.message.clone()
        }
        (LanguageChoicePhase::Ready, _, LanguagePersistence::Unavailable) if !state.shell.demo => {
            lang.tr("language_save_unavailable").into_owned()
        }
        (LanguageChoicePhase::Ready, _, _) => String::new(),
    };
    let retryable = matches!(&model.phase, LanguageChoicePhase::Failed { .. })
        && model.persistence == LanguagePersistence::Available;
    let retry = button(text(lang.tr("language_retry")))
        .style(style_ghost)
        .on_press_maybe(retryable.then_some(Message::RetryLanguageChoice));
    column![
        choices,
        row![text(status).size(12).width(Length::Fill), retry].spacing(8)
    ]
    .spacing(8)
    .into()
}
