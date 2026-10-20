//! Native routing intent capture uses the production command facade and its real refused result.
use crate::demo::capabilities::{ROUTING_INTENT_ENGINE, install_demo_core_capabilities};
use crate::demo::owner::attach_capture_owner;
use crate::state::AppState;
use crate::types::message::Message;
use iced::Task;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_composition::surface_reader::ProductSurfaceReader;
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::core_capabilities::{CoreOperation, ProxyControlOperation};
use infiltrator_contract::parity::FeatureId;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_ports::surface::SurfaceReader;
use infiltrator_test_support::routing_intent_fixtures::RoutingIntentScenarioController;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::page::SurfaceOrigin;
use lurker_core_contract::routing_intent::RoutingIntent;
use std::sync::Arc;

pub(super) fn activate(state: &mut AppState, feature: FeatureId) -> Task<Message> {
    let port = Arc::new(RoutingIntentScenarioController::default());
    if feature == FeatureId::ShellRoutingIntentAuthentication {
        port.reject_requests(Failure::new(
            ErrorCode::Authentication,
            "Controller token rejected",
            false,
        ));
    }
    let runtime = tokio_application_runtime().expect("capture runtime");
    let application = CoreApplication::new_with_overview(
        port.clone(),
        port.clone(),
        port.clone(),
        runtime.clone(),
    );
    // The routing intent change is a real core command. The owner publishes
    // the capability matrix that admits it and the command facade carries the
    // routing intent port, so the surface observes the owner's real refusal
    // instead of a locally invented one.
    let capabilities = install_demo_core_capabilities(
        &application,
        ROUTING_INTENT_ENGINE,
        [
            CoreOperation::ProxyControl(ProxyControlOperation::SetRoutingIntent),
            CoreOperation::ProxyControl(ProxyControlOperation::GetRoutingIntent),
        ],
    );
    application.install_command_handler(Arc::new(
        CommandApplication::new()
            .with_core_capabilities(capabilities)
            .with_routing_intent_port(port.clone()),
    ));
    let ready = application.clone();
    runtime.block_on(Box::pin(async move {
        assert!(ready.adopt_if_running().await.expect("fixture readiness"));
    }));
    attach_capture_owner(state, application.clone());
    // The owner publishes the routing intent it really observed. A surface may
    // only offer a routing choice the owner really read, so the capture reads
    // it through the same reader the product surface uses.
    let reader = ProductSurfaceReader::new(
        Arc::new(application),
        SurfaceKind::IcedDesktop,
        HostKind::Desktop,
    )
    .with_routing_intent_port(port);
    let revision = state.surface.revision() + 1;
    Task::perform(
        async move {
            let mut snapshot = reader
                .read()
                .await
                .expect("actual routing intent observation");
            snapshot.origin = SurfaceOrigin::Demo;
            snapshot.revision = revision;
            snapshot
        },
        |snapshot| Message::SurfaceSnapshotUpdated(Box::new(snapshot)),
    )
    .chain(Task::done(Message::SetRoutingIntent(
        RoutingIntent::RouteAllThroughProxy,
    )))
}
