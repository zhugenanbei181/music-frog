//! Mihomo REST adapter for the runtime-neutral OverviewReader port.

use crate::client::MihomoClient;
use infiltrator_contract::command::ProxyMode;
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_ports::error::PortError;
use infiltrator_ports::overview::{OverviewReader, OverviewSample};

/// Reads the minimal set of controller endpoints required by the Overview
/// application service. The secret remains private inside `MihomoClient`.
#[derive(Clone)]
pub struct ControllerOverviewReader {
    client: MihomoClient,
}

impl ControllerOverviewReader {
    pub fn new(client: MihomoClient) -> Self {
        Self { client }
    }

    pub fn client(&self) -> MihomoClient {
        self.client.clone()
    }
}

#[async_trait::async_trait]
impl OverviewReader for ControllerOverviewReader {
    async fn sample(&self) -> Result<OverviewSample, PortError> {
        let connections = self
            .client
            .get_connections()
            .await
            .map_err(|error| PortError::Network(error.to_string()))?;
        let version = self
            .client
            .get_version()
            .await
            .ok()
            .map(|value| value.version);
        let mode = self
            .client
            .get_config()
            .await
            .ok()
            .and_then(|value| ProxyMode::from_wire(&value.mode));
        let memory_bytes = tokio::time::timeout(
            std::time::Duration::from_millis(250),
            self.client.get_memory(),
        )
        .await
        .ok()
        .and_then(Result::ok)
        .map(|value| value.in_use);
        let sampled_at_epoch_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()
            .and_then(|duration| i64::try_from(duration.as_millis()).ok());

        Ok(OverviewSample {
            lifecycle: CoreLifecycle::Running,
            mode,
            upload_total: connections.upload_total,
            download_total: connections.download_total,
            active_connections: u32::try_from(connections.connections.len()).unwrap_or(u32::MAX),
            memory_bytes,
            core_version: version,
            sampled_at_epoch_ms,
        })
    }

    async fn set_mode(&self, mode: ProxyMode) -> Result<ProxyMode, PortError> {
        self.client
            .patch_config(serde_json::json!({"mode": mode.to_wire()}))
            .await
            .map_err(|error| PortError::Network(error.to_string()))?;
        let actual = self
            .client
            .get_config()
            .await
            .map_err(|error| PortError::Network(error.to_string()))?;
        let actual = ProxyMode::from_wire(&actual.mode).ok_or_else(|| {
            PortError::Failed(format!("unknown controller mode: {}", actual.mode))
        })?;
        if actual != mode {
            return Err(PortError::Failed(format!(
                "内核拒绝模式切换：仍为 {}",
                actual.to_wire(),
            )));
        }
        Ok(actual)
    }
}
