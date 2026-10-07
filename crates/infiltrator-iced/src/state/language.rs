//! TEA language adapter commits only the shared durable terminal result.
use crate::state::AppState;
use crate::types::message::Message;
use iced::Task;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::language::{LanguagePreference, LanguageSettingsSnapshot};
use infiltrator_shared::locales::resolve_language_code;
use std::env;

impl AppState {
    pub(crate) fn observe_language_settings(&mut self, snapshot: &LanguageSettingsSnapshot) {
        self.shell.language_choice.observe(snapshot);
        if env::var("INFILTRATOR_LANG").is_ok() && !self.shell.language_user_selected {
            return;
        }
        if let Some(preference) = self.shell.language_choice.applied {
            self.shell.lang = resolve_language_code(preference.as_setting());
        }
    }
    pub(crate) fn set_language_choice(&mut self, value: String) -> Task<Message> {
        if self.shell.language_choice.pending.is_some() {
            return Task::none();
        }
        let preference = match LanguagePreference::parse(&value) {
            Ok(preference) => preference,
            Err(failure) => {
                self.shell.language_choice.failure = Some(failure);
                return Task::none();
            }
        };
        if self.shell.demo && self.commands.is_none() {
            self.shell.language_choice.applied = Some(preference);
            self.shell.language_user_selected = true;
            self.shell.lang = resolve_language_code(preference.as_setting());
            return Task::none();
        }
        let Some(application) = self.commands.clone() else {
            self.shell.language_choice.failure = Some(Failure::new(
                ErrorCode::NotReady,
                "language settings cannot be saved right now",
                true,
            ));
            return Task::none();
        };
        let pending = match self.shell.language_choice.begin(preference) {
            Ok(pending) => pending,
            Err(failure) => {
                self.shell.language_choice.failure = Some(failure);
                return Task::none();
            }
        };
        Task::perform(
            async move {
                match (application
                    .execute(CommandIntent::SetLanguage { preference })
                    .await)
                    .into_unit()
                {
                    Ok(()) => Ok(()),
                    Err(failure) => Err(failure),
                }
            },
            move |result| Message::LanguageChoiceApplied {
                token: pending.token,
                result,
            },
        )
    }
    pub(crate) fn finish_language_choice(&mut self, token: u64, result: Result<(), Failure>) {
        if let Some(Ok(preference)) = self.shell.language_choice.finish(token, result) {
            self.shell.language_user_selected = true;
            self.shell.lang = resolve_language_code(preference.as_setting());
        }
    }
}
