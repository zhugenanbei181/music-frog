//! Mihomo REST adapter for the runtime-neutral OverviewReader port.

use crate::client::MihomoClient;
use crate::runtime_gateway::network_error;
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_ports::error::PortError;
use infiltrator_ports::overview::{OverviewReader, OverviewSample};
use std::time;
use tokio::time::timeout;

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
        let connections = self.client.get_connections().await.map_err(network_error)?;
        let version = self
            .client
            .get_version()
            .await
            .ok()
            .map(|value| value.version);
        let memory_bytes = timeout(time::Duration::from_millis(250), self.client.get_memory())
            .await
            .ok()
            .and_then(Result::ok)
            .map(|value| value.in_use);
        let sampled_at_epoch_ms = time::SystemTime::now()
            .duration_since(time::UNIX_EPOCH)
            .ok()
            .and_then(|duration| i64::try_from(duration.as_millis()).ok());

        Ok(OverviewSample {
            lifecycle: CoreLifecycle::Running,
            upload_total: connections.upload_total,
            download_total: connections.download_total,
            active_connections: u32::try_from(connections.connections.len()).unwrap_or(u32::MAX),
            memory_bytes,
            core_version: version,
            sampled_at_epoch_ms,
        })
    }
}
