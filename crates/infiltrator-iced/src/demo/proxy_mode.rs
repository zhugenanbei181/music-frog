//! Native mode capture uses the production command facade and its real refused terminal result.
use crate::state::AppState;
use crate::types::message::Message;
use iced::Task;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::proxy_mode_fixtures::ModeScenarioController;
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::parity::FeatureId;
use infiltrator_contract::surface::SurfaceKind;
use std::sync::Arc;

pub(super) fn activate(state: &mut AppState, feature: FeatureId) -> Task<Message> {
    let port = Arc::new(ModeScenarioController::default());
    if feature == FeatureId::ShellProxyModeAuthentication {
        port.reject_with(Some(Failure::new(
            ErrorCode::Authentication,
            "Controller token rejected",
            false,
        )));
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
    state.apply_shared_surface_snapshot(
        port.snapshot(application.snapshot(), SurfaceKind::IcedDesktop),
    );
    state.commands = Some(application);
    state.update(Message::SetProxyMode("global".into()))
}
