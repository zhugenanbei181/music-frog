use crate::error::PortError;
use async_trait::async_trait;
use infiltrator_contract::command::ProxyMode;
use infiltrator_contract::snapshot::CoreLifecycle;

/// One transport-independent observation of the running core.
#[derive(Clone, Debug, PartialEq)]
pub struct OverviewSample {
    pub lifecycle: CoreLifecycle,
    pub mode: Option<ProxyMode>,
    pub upload_total: u64,
    pub download_total: u64,
    pub active_connections: u32,
    pub memory_bytes: Option<u64>,
    pub core_version: Option<String>,
    pub sampled_at_epoch_ms: Option<i64>,
}

/// Reads the data needed by an Overview projection and applies proxy-mode
/// commands. HTTP/WebSocket clients implement this port; application code
/// does not know their concrete types.
#[async_trait]
pub trait OverviewReader: Send + Sync {
    async fn sample(&self) -> Result<OverviewSample, PortError>;
    async fn set_mode(&self, mode: ProxyMode) -> Result<ProxyMode, PortError>;
}
