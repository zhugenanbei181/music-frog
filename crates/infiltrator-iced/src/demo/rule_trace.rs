//! Capture executes the same shared tracer command and reader as the interactive product.
use crate::demo::capabilities::{
    DemoMihomoCaptureCore, MIHOMO_CAPTURE_ENGINE, install_demo_core_capabilities,
};
use crate::demo::owner::attach_capture_owner;
use crate::state::AppState;
use crate::types::message::Message;
use crate::view::rules_tracer::TRACER_SCROLL_ID;
use iced::Task;
use iced::widget::Id;
use iced::widget::operation::snap_to;
use iced::widget::scrollable::RelativeOffset;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::rule_tracer_application::RuleTracerApplication;
use infiltrator_composition::mihomo_configuration::for_mihomo_store;
use infiltrator_composition::mihomo_configuration::mihomo_rule_trace_fixture;
use infiltrator_composition::mihomo_configuration::profiles_for_mihomo;
use infiltrator_composition::surface_reader::ProductSurfaceReader;
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::core_capabilities::{CoreOperation, RulesOperation};
use infiltrator_contract::parity::FeatureId;
use infiltrator_contract::rules_workspace::RulesTab;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_ports::surface::SurfaceReader;
use lurker_core_contract::page::SurfaceOrigin;
use std::sync::{Arc, atomic::Ordering};

pub(super) fn activate(state: &mut AppState, feature: FeatureId) -> Task<Message> {
    let confirm = feature == FeatureId::RulesOverrideEditor;
    let port = mihomo_rule_trace_fixture();
    let owner = RuleTracerApplication::new();
    owner.set_override_port(port.clone());
    // The tracer reads a Mihomo profile document, and the shared rule-trace
    // command refuses a workspace that belongs to another engine: the capture
    // core therefore speaks the Mihomo engine, exactly as the product core
    // does.
    let core = CoreApplication::new(
        Arc::new(DemoMihomoCaptureCore),
        Arc::new(DemoMihomoCaptureCore),
        tokio_application_runtime().expect("isolated tracer capture runtime"),
    );
    // The tracer runs the shared rule commands, so the owner publishes the
    // capability matrix that admits them. Without it the capability gate
    // refuses the trace and the capture never observes the owner's report.
    let capabilities = install_demo_core_capabilities(
        &core,
        MIHOMO_CAPTURE_ENGINE,
        [
            CoreOperation::Rules(RulesOperation::Trace),
            CoreOperation::Rules(RulesOperation::Apply),
        ],
    );
    core.install_command_handler(Arc::new(
        CommandApplication::new()
            .with_core_capabilities(capabilities)
            // The trace command reads the active profile workspace, so the
            // facade carries the same profile application the reader uses.
            .with_profile(profiles_for_mihomo(port.clone()))
            .with_rule_tracer(owner.clone()),
    ));
    let reader = Arc::new(
        ProductSurfaceReader::new(
            Arc::new(core.clone()),
            SurfaceKind::IcedDesktop,
            HostKind::Desktop,
        )
        .with_rule_tracer(owner)
        .with_profiles(profiles_for_mihomo(port.clone()))
        .with_configuration(for_mihomo_store(port.clone())),
    );
    attach_capture_owner(state, core);
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
