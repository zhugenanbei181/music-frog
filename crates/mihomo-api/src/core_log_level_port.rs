//! Mihomo implementation of the engine-neutral core log-level port.

use crate::core_log_level::{decode_mihomo_log_level, encode_mihomo_log_level};
use async_trait::async_trait;
use infiltrator_contract::command::CoreLogLevel;
use infiltrator_ports::core_log_level::CoreLogLevelPort;
use lurker_core_ports::error::PortError;
use mihomo_driver::api::client::MihomoClient;
use mihomo_driver::network_error::network_error;

/// Product core-log-level port over one controller client.
#[derive(Clone)]
pub struct MihomoCoreLogLevelAdapter {
    client: MihomoClient,
}

impl MihomoCoreLogLevelAdapter {
    pub fn new(client: MihomoClient) -> Self {
        Self { client }
    }
}

#[async_trait]
impl CoreLogLevelPort for MihomoCoreLogLevelAdapter {
    async fn set_core_log_level(&self, level: CoreLogLevel) -> Result<CoreLogLevel, PortError> {
        self.client
            .patch_config(serde_json::json!({
                "log-level": encode_mihomo_log_level(level),
            }))
            .await
            .map_err(network_error)?;

        let observed = self.client.get_config().await.map_err(network_error)?;
        decode_mihomo_log_level(&observed.log_level).ok_or_else(|| {
            PortError::Failed(format!(
                "Mihomo returned an unsupported log level: {}",
                observed.log_level
            ))
        })
    }
}

#[cfg(test)]
#[path = "controller_status_tests.rs"]
mod controller_status_tests;
