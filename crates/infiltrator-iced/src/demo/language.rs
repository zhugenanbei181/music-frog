//! Capture executes the production TEA command and its actual terminal result.
use crate::demo::owner::attach_capture_owner;
use crate::state::AppState;
use crate::types::message::Message;
use iced::Task;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::settings_application::SettingsApplication;
use infiltrator_composition::tokio_application_runtime;
use infiltrator_presentation::language_choice::project_language;
use infiltrator_test_support::language_choice_fixtures::{
    LanguageCaptureProcess, LanguageCaptureStore,
};
use lurker_core_contract::page::SurfaceOrigin;
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
    attach_capture_owner(state, application);
    state.shell.lang = "zh-CN".into();
    state.observe_language_settings(&project_language(Some(&Ok(store.saved()))));

    // The choice is only applied when the owner's own readback agrees with it.
    // The capture therefore reads the owner's persisted settings back after
    // the command's terminal result and publishes them as the owner's next
    // surface snapshot; the surface observes that snapshot like any other.
    let base = state.surface.latest().cloned();
    let revision = state.surface.revision() + 1;
    state
        .update(Message::SetLanguage("en-US".into()))
        .then(move |message| {
            let store = store.clone();
            let base = base.clone();
            Task::done(message).chain(Task::perform(
                async move {
                    let mut snapshot = base.expect("the capture owner published a snapshot");
                    snapshot.language_settings = project_language(Some(&Ok(store.saved())));
                    snapshot.origin = SurfaceOrigin::Demo;
                    snapshot.revision = revision;
                    snapshot
                },
                |snapshot| Message::SurfaceSnapshotUpdated(Box::new(snapshot)),
            ))
        })
}
