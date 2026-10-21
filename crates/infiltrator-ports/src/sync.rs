//! Runtime-neutral WebDAV synchronization port.

use async_trait::async_trait;
use infiltrator_contract::sync::{SyncConflict, SyncProgress, SyncReport, SyncTransferReport};
use infiltrator_domain::settings::WebDavConfig;
use lurker_core_contract::capability::Capability;
use lurker_core_ports::error::PortError;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct SyncRequest {
    pub config: WebDavConfig,
    pub configs_dir: Option<String>,
}

/// The sync owner's snapshot of the conflict it currently holds.
///
/// The owner is the only source of this fact: the snapshot names the pending
/// conflict and the owner's revision of it, so a resolution can be fenced
/// against the exact conflict the caller observed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncConflictSnapshot {
    /// The conflict the owner holds.
    pub conflict: SyncConflict,
    /// The owner's revision of this pending conflict.
    pub revision: u64,
}

pub trait SyncProgressSink: Send + Sync {
    fn progress(&self, progress: SyncProgress);
    fn is_cancelled(&self) -> bool;
}

pub struct SyncTransferRequest {
    pub config: WebDavConfig,
    pub configs_dir: Option<String>,
    pub observer: Arc<dyn SyncProgressSink>,
}

#[async_trait]
pub trait SyncPort: Send + Sync {
    async fn test(&self, config: WebDavConfig) -> Result<usize, PortError>;
    async fn sync(&self, request: SyncRequest) -> Result<SyncReport, PortError>;
    async fn upload(&self, request: SyncTransferRequest) -> Result<SyncTransferReport, PortError>;
    async fn download(&self, request: SyncTransferRequest)
    -> Result<SyncTransferReport, PortError>;

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

    /// The conflict the owner currently holds, if any.
    ///
    /// The owner is the single source of this fact. An adapter that cannot
    /// report its pending conflict answers with a typed unsupported instead of
    /// claiming that no conflict exists.
    async fn current_conflict(&self) -> Result<Option<SyncConflictSnapshot>, PortError> {
        Err(PortError::unsupported(
            Capability::WebDavSync,
            "this sync adapter does not expose its pending conflict",
        ))
    }
}
