//! Capture the real typed options read and denied filter transaction through TEA.
use crate::state::AppState;
use crate::types::message::Message;
use crate::types::options::EditorPane;
use iced::Task;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::filter_capture_store::{
    FILTER_POLICY, FILTER_PROFILE, FilterCaptureStore,
};
use infiltrator_application::language_choice_fixtures::LanguageCaptureProcess;
use infiltrator_application::profile_application::ProfileApplication;
use infiltrator_application::surface_reader::ApplicationSurfaceReader;
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::parity::FeatureId;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::SurfaceOrigin;
use infiltrator_ports::surface::SurfaceReader;
use std::path::PathBuf;
use std::sync::{Arc, atomic::Ordering};

pub(super) fn activate(state: &mut AppState) -> Task<Message> {
    let store = Arc::new(FilterCaptureStore::default());
    store.deny_write.store(true, Ordering::SeqCst);
    let core = CoreApplication::new(
        Arc::new(LanguageCaptureProcess),
        Arc::new(LanguageCaptureProcess),
        tokio_application_runtime().expect("isolated filter runtime"),
    );
    let profiles = ProfileApplication::new(store);
    core.install_command_handler(Arc::new(
        CommandApplication::new().with_profile(profiles.clone()),
    ));
    state.commands = Some(core.clone());
    let reader = ApplicationSurfaceReader::new(
        Arc::new(core.clone()),
        SurfaceKind::IcedDesktop,
        HostKind::Desktop,
    )
    .with_profiles(profiles);
    let revision = state.surface.revision() + 1;
    state.editor.editor_path = Some(PathBuf::from(format!(
        "/isolated-filter/{FILTER_PROFILE}.yaml"
    )));
    Task::perform(
        async move {
            core.execute(CommandIntent::LoadProfileDocument {
                profile: Some(FILTER_PROFILE.into()),
            })
            .await
            .into_output()
            .and_then(|output| output.into_profile_document())
            .expect("actual isolated profile document");
            let mut snapshot = reader.read().await.expect("actual isolated surface");
            snapshot.origin = SurfaceOrigin::Demo;
            snapshot.revision = revision;
            snapshot
        },
        |snapshot| Message::SurfaceSnapshotUpdated(Box::new(snapshot)),
    )
    .chain(Task::done(Message::SetEditorPane(EditorPane::Filter)))
}
pub(super) fn update(state: &mut AppState, message: Message) -> Task<Message> {
    let loaded = matches!(&message, Message::ProfileFilterLoaded { result: Ok(_), .. });
    let task = state.update(message);
    if loaded
        && state.shell.capture_scenario == Some(FeatureId::ProfilesFilterEditor)
        && state.editor.filter_editor.can_edit()
        && !state.editor.filter_editor.dirty()
    {
        task.chain(Task::done(Message::UpdateFilterInclude("HK".into())))
            .chain(Task::done(Message::UpdateFilterAdvancedPolicy(
                FILTER_POLICY.into(),
            )))
            .chain(Task::done(Message::SaveProfileFilter))
    } else {
        task
    }
}
