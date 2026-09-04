//! Concrete adapter wiring for native and cross-platform application hosts.
//!
//! The application crate owns orchestration and private runtime details. This
//! crate owns the decision that a particular product composition uses the
//! Mihomo REST adapter. UI crates may consume the returned application pump,
//! but do not construct `MihomoClient` themselves.

use infiltrator_application::overview::{OverviewConfig, OverviewPump, UnavailableOverviewReader};
use infiltrator_ports::error::PortError;
use infiltrator_ports::overview::OverviewReader;
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
