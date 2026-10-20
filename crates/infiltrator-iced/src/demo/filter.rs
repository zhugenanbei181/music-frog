//! Capture the real typed options read and denied filter transaction through TEA.
use crate::demo::capabilities::{CAPTURE_ENGINE, install_demo_core_capabilities};
use crate::demo::owner::attach_capture_owner;
use crate::state::AppState;
use crate::types::message::Message;
use crate::types::options::EditorPane;
use iced::Task;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_composition::mihomo_configuration::profiles_for_mihomo;
use infiltrator_composition::surface_reader::ProductSurfaceReader;
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::core_capabilities::{CoreOperation, ProfileOperation};
use infiltrator_contract::parity::FeatureId;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_ports::surface::SurfaceReader;
use infiltrator_test_support::filter_capture_store::{
    FILTER_POLICY, FILTER_PROFILE, FilterCaptureStore,
};
use infiltrator_test_support::language_choice_fixtures::LanguageCaptureProcess;
use lurker_core_contract::page::SurfaceOrigin;
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
    // The filter transaction is a real core command: the owner publishes the
    // capability that admits it, so the capture observes the owner's real
    // refusal instead of a capability-gate refusal.
    let capabilities = install_demo_core_capabilities(
        &core,
        CAPTURE_ENGINE,
        [CoreOperation::Profile(ProfileOperation::ApplyFeature)],
    );
    let profiles = profiles_for_mihomo(store);
    core.install_command_handler(Arc::new(
        CommandApplication::new()
            .with_core_capabilities(capabilities)
            .with_profile(profiles.clone()),
    ));
    attach_capture_owner(state, core.clone());
    let commands = state
        .command_ingress()
        .expect("attached capture application")
        .clone();
    let reader = ProductSurfaceReader::new(
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
            commands
                .execute(CommandIntent::LoadProfileDocument {
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
