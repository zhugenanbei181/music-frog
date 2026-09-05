//! Runtime-neutral WebDAV synchronization port.

use async_trait::async_trait;
use infiltrator_contract::sync::{SyncProgress, SyncReport, SyncTransferReport};
use infiltrator_domain::settings::WebDavConfig;
use std::sync::Arc;

use crate::error::PortError;

#[derive(Clone, Debug)]
pub struct SyncRequest {
    pub config: WebDavConfig,
    pub configs_dir: Option<String>,
}

pub trait SyncProgressSink: Send + Sync {
    fn progress(&self, progress: SyncProgress);
    fn is_cancelled(&self) -> bool;
}

pub struct SyncTransferRequest {
    pub config: WebDavConfig,
    pub configs_dir: Option<String>,
    pub runtime_present: bool,
    pub observer: Arc<dyn SyncProgressSink>,
}

#[async_trait]
pub trait SyncPort: Send + Sync {
    async fn test(&self, config: WebDavConfig) -> Result<usize, PortError>;
    async fn sync(&self, request: SyncRequest) -> Result<SyncReport, PortError>;
    async fn upload(&self, request: SyncTransferRequest) -> Result<SyncTransferReport, PortError>;
    async fn download(
        &self,
        request: SyncTransferRequest,
    ) -> Result<SyncTransferReport, PortError>;
}
