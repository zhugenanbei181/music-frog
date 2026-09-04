//! Android host/application composition for the 0.30 seam.

use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::overview::UnavailableOverviewReader;
use infiltrator_ports::core_process::CoreReadiness;
use infiltrator_ports::error::PortError;
use infiltrator_ports::overview::OverviewReader;
use mihomo_api::client::MihomoClient;
use mihomo_api::overview::ControllerOverviewReader;
use mihomo_api::readiness::ControllerReadiness;
use mihomo_platform::android_bridge::AndroidBridge;
use std::sync::Arc;

use crate::runtime::AndroidBridgeAdapter;

/// Assemble the shared application service with Android's bridge-backed Core
/// process port and the Mihomo controller readiness adapter.
pub fn core_application<B>(
    bridge: B,
    controller_url: impl Into<String>,
    secret: Option<String>,
) -> CoreApplication
where
    B: AndroidBridge + 'static,
{
    let controller_url = controller_url.into();
    let process = Arc::new(AndroidBridgeAdapter::new(bridge));
    let readiness: Arc<dyn CoreReadiness> = Arc::new(ControllerReadiness::new(
        controller_url.clone(),
        secret.clone(),
    ));
    let reader: Arc<dyn OverviewReader> = match MihomoClient::new(&controller_url, secret.clone()) {
        Ok(client) => Arc::new(ControllerOverviewReader::new(client)),
        Err(error) => Arc::new(UnavailableOverviewReader::new(PortError::Network(
            error.to_string(),
        ))),
    };
    let runtime = infiltrator_composition::tokio_application_runtime()
        .expect("Tokio application runtime must be constructible");
    CoreApplication::new_with_overview(process, readiness, reader, runtime)
}
