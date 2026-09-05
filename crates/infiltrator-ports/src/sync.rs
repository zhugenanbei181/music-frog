//! Runtime-neutral WebDAV synchronization port.

use async_trait::async_trait;
use infiltrator_contract::sync::SyncReport;
use infiltrator_domain::settings::WebDavConfig;

use crate::error::PortError;

#[derive(Clone, Debug)]
pub struct SyncRequest {
    pub config: WebDavConfig,
    pub configs_dir: Option<String>,
}

#[async_trait]
pub trait SyncPort: Send + Sync {
    async fn test(&self, config: WebDavConfig) -> Result<usize, PortError>;
    async fn sync(&self, request: SyncRequest) -> Result<SyncReport, PortError>;
}
