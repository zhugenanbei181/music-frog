//! Native routing intent capture uses the production command facade and its real refused result.
use crate::demo::owner::attach_capture_owner_with_snapshot;
use crate::state::AppState;
use crate::types::message::Message;
use iced::Task;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::parity::FeatureId;
use infiltrator_contract::surface::SurfaceKind;
use infiltrator_test_support::routing_intent_fixtures::RoutingIntentScenarioController;
use lurker_core_contract::error::{ErrorCode, Failure};
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
    let ready = application.clone();
    runtime.block_on(Box::pin(async move {
        assert!(ready.adopt_if_running().await.expect("fixture readiness"));
    }));
    let snapshot = port.snapshot(application.snapshot(), SurfaceKind::IcedDesktop);
    attach_capture_owner_with_snapshot(state, application, snapshot);
    state.update(Message::SetRoutingIntent(
        RoutingIntent::RouteAllThroughProxy,
    ))
}
