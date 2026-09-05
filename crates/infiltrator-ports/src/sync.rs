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

    /// Read a conflict file after validating that it stays inside the
    /// resolved configs directory supplied by the host/application.
    async fn read_conflict(
        &self,
        configs_dir: String,
        remote_path: String,
    ) -> Result<String, PortError> {
        let _ = (configs_dir, remote_path);
        Err(PortError::Failed(
            "sync conflict reads are not supported by this adapter".to_string(),
        ))
    }

    /// Remove a conflict file after the host adapter validates its path.
    async fn delete_conflict(
        &self,
        configs_dir: String,
        remote_path: String,
    ) -> Result<(), PortError> {
        let _ = (configs_dir, remote_path);
        Err(PortError::Failed(
            "sync conflict deletion is not supported by this adapter".to_string(),
        ))
    }
}
