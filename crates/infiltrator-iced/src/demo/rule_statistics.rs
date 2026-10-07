//! Capture the actual reader and independent native cleanup review before any write.
use crate::state::AppState;
use crate::types::message::Message;
use iced::Task;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::configuration_application::ConfigurationApplication;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::language_choice_fixtures::LanguageCaptureProcess;
use infiltrator_application::rule_statistics_workbench::{StatisticsAction, StatisticsTab};
use infiltrator_application::rule_trace_fixtures::RuleTraceStore;
use infiltrator_application::rule_tracer_application::RuleTracerApplication;
use infiltrator_application::surface_reader::ApplicationSurfaceReader;
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::parity::FeatureId;
use infiltrator_contract::rule_trace_run::{RuleTraceOperationId, RuleTraceRequest};
use infiltrator_contract::rule_tracer::TrafficContextSnapshot;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::SurfaceOrigin;
use infiltrator_ports::surface::SurfaceReader;
use std::sync::Arc;

pub(super) fn activate(state: &mut AppState) -> Task<Message> {
    let store = Arc::new(RuleTraceStore::default());
    let owner = RuleTracerApplication::new();
    owner.set_override_port(store.clone());
    let core = CoreApplication::new(
        Arc::new(LanguageCaptureProcess),
        Arc::new(LanguageCaptureProcess),
        tokio_application_runtime().expect("isolated statistics runtime"),
    );
    core.install_command_handler(Arc::new(
        CommandApplication::new().with_rule_tracer(owner.clone()),
    ));
    let reader = ApplicationSurfaceReader::new(
        Arc::new(core.clone()),
        SurfaceKind::IcedDesktop,
        HostKind::Desktop,
    )
    .with_rule_tracer(owner.clone())
    .with_configuration(ConfigurationApplication::new(store));
    state.commands = Some(core);
    let revision = state.surface.revision() + 1;
    Task::perform(
        async move {
            owner
                .simulate(
                    RuleTraceOperationId(1),
                    RuleTraceRequest {
                        query: "special.com".into(),
                        context: TrafficContextSnapshot {
                            src_ip: Some("192.0.2.1".into()),
                            ..Default::default()
                        },
                    },
                    None,
                )
                .await
                .expect("actual trace statistics");
            let mut snapshot = reader.read().await.expect("actual statistics page");
            snapshot.origin = SurfaceOrigin::Demo;
            snapshot.revision = revision;
            snapshot
        },
        |snapshot| Message::SurfaceSnapshotUpdated(Box::new(snapshot)),
    )
}
pub(super) fn update(state: &mut AppState, message: Message) -> Task<Message> {
    let observed = matches!(message, Message::SurfaceSnapshotUpdated(_));
    let task = state.update(message);
    if observed
        && state.shell.capture_scenario == Some(FeatureId::RulesStatisticsInspector)
        && state.editor.rule_hit_audit.current()
        && state.editor.rule_hit_audit.confirmation.is_none()
    {
        task.chain(Task::done(Message::RuleStatistics(StatisticsAction::Tab(
            StatisticsTab::Inactive,
        ))))
        .chain(Task::done(Message::RuleStatistics(
            StatisticsAction::Inspect,
        )))
        .chain(Task::done(Message::RuleStatistics(
            StatisticsAction::PrepareCleanup,
        )))
    } else {
        task
    }
}
