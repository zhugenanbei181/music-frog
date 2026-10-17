//! Capture a real source-bound staged edit and the actual denied commit terminal.
use crate::demo::owner::attach_capture_owner;
use crate::state::AppState;
use crate::types::message::Message;
use iced::Task;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::rule_list_application::RuleListApplication;
use infiltrator_composition::mihomo_configuration::for_mihomo_store;
use infiltrator_composition::mihomo_configuration::mihomo_rule_trace_fixture;
use infiltrator_composition::mihomo_configuration::profiles_for_mihomo;
use infiltrator_composition::surface_reader::ProductSurfaceReader;
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::parity::FeatureId;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_ports::surface::SurfaceReader;
use infiltrator_test_support::language_choice_fixtures::LanguageCaptureProcess;
use lurker_core_contract::page::SurfaceOrigin;
use std::sync::{Arc, atomic::Ordering};

pub(super) fn activate(state: &mut AppState) -> Task<Message> {
    let store = mihomo_rule_trace_fixture();
    store.deny_write.store(true, Ordering::SeqCst);
    let core = CoreApplication::new(
        Arc::new(LanguageCaptureProcess),
        Arc::new(LanguageCaptureProcess),
        tokio_application_runtime().expect("isolated rule-list runtime"),
    );
    core.install_command_handler(Arc::new(
        CommandApplication::new().with_rule_list(RuleListApplication::new(store.clone())),
    ));
    let reader = ProductSurfaceReader::new(
        Arc::new(core.clone()),
        SurfaceKind::IcedDesktop,
        HostKind::Desktop,
    )
    .with_profiles(profiles_for_mihomo(store.clone()))
    .with_configuration(for_mihomo_store(store));
    attach_capture_owner(state, core);
    let revision = state.surface.revision() + 1;
    Task::perform(
        async move {
            let mut snapshot = reader
                .read()
                .await
                .expect("actual isolated complete rule page");
            snapshot.origin = SurfaceOrigin::Demo;
            snapshot.revision = revision;
            snapshot
        },
        |snapshot| Message::SurfaceSnapshotUpdated(Box::new(snapshot)),
    )
}

/// Resolve the captured control from the draft the production reader adopted.
pub(super) fn update(state: &mut AppState, message: Message) -> Task<Message> {
    let observed = matches!(message, Message::SurfaceSnapshotUpdated(_));
    let task = state.update(message);
    if observed
        && state.shell.capture_scenario == Some(FeatureId::RulesListEditor)
        && state.editor.rule_list.editable()
        && !state.editor.rule_list.dirty()
        && state.editor.rule_list.failure().is_none()
        && let Some(id) = state.editor.rule_list.row_id(0)
    {
        task.chain(Task::done(Message::ToggleRuleEnabled(id)))
            .chain(Task::done(Message::SaveRules))
    } else {
        task
    }
}
