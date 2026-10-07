//! Capture executes the production TEA command and its actual terminal result.
use crate::state::AppState;
use crate::types::message::Message;
use iced::Task;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::language_choice::project_language;
use infiltrator_application::language_choice_fixtures::{
    LanguageCaptureProcess, LanguageCaptureStore,
};
use infiltrator_application::settings_application::SettingsApplication;
use infiltrator_composition::tokio_application_runtime;
use std::sync::Arc;

pub(super) fn activate(state: &mut AppState) -> Task<Message> {
    let store = Arc::new(LanguageCaptureStore::default());
    let runtime = tokio_application_runtime().expect("capture runtime");
    let application = CoreApplication::new(
        Arc::new(LanguageCaptureProcess),
        Arc::new(LanguageCaptureProcess),
        runtime.clone(),
    );
    application.install_command_handler(Arc::new(
        CommandApplication::new().with_settings(SettingsApplication::new(store.clone())),
    ));
    state.commands = Some(application);
    state.shell.lang = "zh-CN".into();
    state.observe_language_settings(&project_language(Some(&Ok(store.saved()))));
    state.update(Message::SetLanguage("en-US".into()))
}
