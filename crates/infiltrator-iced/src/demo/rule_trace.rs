//! Capture executes the same shared tracer command and reader as the interactive product.
use crate::state::AppState;
use crate::types::message::Message;
use crate::view::rules_tracer::TRACER_SCROLL_ID;
use iced::Task;
use iced::widget::Id;
use iced::widget::operation::snap_to;
use iced::widget::scrollable::RelativeOffset;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::configuration_application::ConfigurationApplication;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::language_choice_fixtures::LanguageCaptureProcess;
use infiltrator_application::profile_application::ProfileApplication;
use infiltrator_application::rule_trace_fixtures::RuleTraceStore;
use infiltrator_application::rule_tracer_application::RuleTracerApplication;
use infiltrator_application::surface_reader::ApplicationSurfaceReader;
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::parity::FeatureId;
use infiltrator_contract::rules_workspace::RulesTab;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::SurfaceOrigin;
use infiltrator_ports::surface::SurfaceReader;
use std::sync::{Arc, atomic::Ordering};

pub(super) fn activate(state: &mut AppState) -> Task<Message> {
    let confirm = state.shell.capture_scenario == Some(FeatureId::RulesOverrideEditor);
    let port = Arc::new(RuleTraceStore::default());
    let owner = RuleTracerApplication::new();
    owner.set_override_port(port.clone());
    let core = CoreApplication::new(
        Arc::new(LanguageCaptureProcess),
        Arc::new(LanguageCaptureProcess),
        tokio_application_runtime().expect("isolated tracer capture runtime"),
    );
    core.install_command_handler(Arc::new(
        CommandApplication::new().with_rule_tracer(owner.clone()),
    ));
    let reader = Arc::new(
        ApplicationSurfaceReader::new(
            Arc::new(core.clone()),
            SurfaceKind::IcedDesktop,
            HostKind::Desktop,
        )
        .with_rule_tracer(owner)
        .with_profiles(ProfileApplication::new(port.clone()))
        .with_configuration(ConfigurationApplication::new(port.clone())),
    );
    state.commands = Some(core);
    let _ = state.update(Message::SetRulesTab(RulesTab::Tracer));
    let _ = state.update(Message::UpdateRulesTracerInput("google.com".into()));
    let _ = state.update(Message::UpdateTracerSourceIp("192.0.2.1".into()));
    let revision = state.surface.revision() + 1;
    state.update(Message::RunRulesTracer).then(move |message| {
        let reader = reader.clone();
        let port = port.clone();
        Task::done(message).chain(
            Task::perform(
                async move {
                    let mut snapshot = reader.read().await.expect("actual tracer capture report");
                    assert_eq!(port.rule_loads.load(Ordering::SeqCst), 1);
                    snapshot.origin = SurfaceOrigin::Demo;
                    snapshot.revision = revision;
                    snapshot
                },
                |snapshot| Message::SurfaceSnapshotUpdated(Box::new(snapshot)),
            )
            .chain(if confirm {
                Task::done(Message::UpdateTracerOverrideTarget("DIRECT".into())).chain(Task::done(
                    Message::ApplyTracerRuleOverride { rule_index: 2 },
                ))
            } else {
                snap_to(Id::new(TRACER_SCROLL_ID), RelativeOffset::END)
            }),
        )
    })
}
