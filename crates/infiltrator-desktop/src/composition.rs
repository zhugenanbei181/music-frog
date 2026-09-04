//! Desktop composition root helpers.
//!
//! This module is the only place in the desktop host that assembles a
//! Tokio-backed application service with a concrete process and controller
//! adapter. UI crates receive the resulting `CoreApplication` handle.

use infiltrator_application::core_application::CoreApplication;
use infiltrator_ports::core_process::CoreReadiness;
use mihomo_api::readiness::ControllerReadiness;

use crate::service::ServiceManager;

/// Build the 0.30 lifecycle application over the desktop process host and
/// Mihomo controller readiness adapter.
pub fn core_application(
    service: &ServiceManager,
    controller_url: impl Into<String>,
    secret: Option<String>,
) -> CoreApplication {
    let readiness: std::sync::Arc<dyn CoreReadiness> =
        std::sync::Arc::new(ControllerReadiness::new(controller_url, secret));
    CoreApplication::new(service.core_process(), readiness)
}
