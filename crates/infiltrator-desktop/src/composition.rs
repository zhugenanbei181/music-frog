//! Desktop composition root helpers.
//!
//! This module is the only place in the desktop host that assembles a
//! Tokio-backed application service with a concrete process and controller
//! adapter. UI crates receive the resulting `CoreApplication` handle.

use infiltrator_application::core_application::CoreApplication;
use mihomo_api::client::MihomoClient;
use mihomo_api::overview::ControllerOverviewReader;
use mihomo_api::readiness::ControllerReadiness;

use crate::service::ServiceManager;

/// Build the 0.30 lifecycle application over the desktop process host and
/// Mihomo controller readiness adapter.
pub fn core_application(
    service: &ServiceManager,
    controller_url: impl Into<String>,
    secret: Option<String>,
) -> anyhow::Result<CoreApplication> {
    let controller_url = controller_url.into();
    let client = MihomoClient::new(&controller_url, secret.clone())?;
    let runtime = infiltrator_composition::tokio_application_runtime()
        .map_err(|error| anyhow::anyhow!(error))?;
    Ok(CoreApplication::new_with_overview(
        service.core_process(),
        std::sync::Arc::new(ControllerReadiness::new(controller_url, secret)),
        std::sync::Arc::new(ControllerOverviewReader::new(client)),
        runtime,
    ))
}
