//! Concrete adapter wiring for native and cross-platform application hosts.
//!
//! The application crate owns orchestration and private runtime details. This
//! crate owns the decision that a particular product composition uses the
//! Mihomo REST adapter. UI crates may consume the returned application pump,
//! but do not construct `MihomoClient` themselves.

use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::overview::{OverviewConfig, OverviewPump, UnavailableOverviewReader};
use infiltrator_ios::{IosBridge, IosHostAdapter};
use infiltrator_ports::error::PortError;
use infiltrator_ports::overview::OverviewReader;
use mihomo_api::client::MihomoClient;
use mihomo_api::overview::ControllerOverviewReader;
use mihomo_api::readiness::ControllerReadiness;
use std::sync::Arc;

/// Build the standard Mihomo-backed Overview pump for a product composition.
pub fn spawn_mihomo_overview(config: OverviewConfig) -> OverviewPump {
    let reader: Arc<dyn OverviewReader> =
        match mihomo_api::client::MihomoClient::new(&config.endpoint, config.secret.clone()) {
            Ok(client) => Arc::new(mihomo_api::overview::ControllerOverviewReader::new(client)),
            Err(error) => Arc::new(UnavailableOverviewReader::new(PortError::Network(
                error.to_string(),
            ))),
        };
    OverviewPump::spawn(reader, config.sample_interval)
}

/// Assemble the shared application for an iOS host. The native bridge is the
/// only iOS-specific input; NetworkExtension and Swift lifecycle details stay
/// behind `IosBridge`.
pub fn ios_core_application<B>(
    bridge: B,
    controller_url: impl Into<String>,
    secret: Option<String>,
) -> Result<CoreApplication, String>
where
    B: IosBridge + 'static,
{
    let controller_url = controller_url.into();
    let client =
        MihomoClient::new(&controller_url, secret.clone()).map_err(|error| error.to_string())?;
    Ok(CoreApplication::new_with_overview(
        std::sync::Arc::new(IosHostAdapter::new(bridge)),
        std::sync::Arc::new(ControllerReadiness::new(controller_url, secret)),
        std::sync::Arc::new(ControllerOverviewReader::new(client)),
    ))
}
