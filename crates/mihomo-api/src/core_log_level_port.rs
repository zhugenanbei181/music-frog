//! Mihomo implementation of the engine-neutral core log-level port.

use crate::client::MihomoClient;
use crate::core_log_level::{decode_mihomo_log_level, encode_mihomo_log_level};
use crate::network_error::network_error;
use async_trait::async_trait;
use infiltrator_contract::command::CoreLogLevel;
use infiltrator_ports::core_log_level::CoreLogLevelPort;
use lurker_core_ports::error::PortError;

#[async_trait]
impl CoreLogLevelPort for MihomoClient {
    async fn set_core_log_level(&self, level: CoreLogLevel) -> Result<CoreLogLevel, PortError> {
        MihomoClient::patch_config(
            self,
            serde_json::json!({
                "log-level": encode_mihomo_log_level(level),
            }),
        )
        .await
        .map_err(network_error)?;

        let observed = MihomoClient::get_config(self)
            .await
            .map_err(network_error)?;
        decode_mihomo_log_level(&observed.log_level).ok_or_else(|| {
            PortError::Failed(format!(
                "Mihomo returned an unsupported log level: {}",
                observed.log_level
            ))
        })
    }
}
