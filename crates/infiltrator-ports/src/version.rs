//! Runtime-neutral core-version delivery port.

use async_trait::async_trait;
use infiltrator_contract::version::{
    CoreArtifactVerification, CoreRelease, CoreReleaseChannel, CoreReleaseSummary,
    CoreRollbackSnapshot, InstalledCoreVersion, VersionDownloadProgress,
};
use std::sync::Arc;

use crate::error::PortError;

pub trait VersionProgressSink: Send + Sync {
    fn progress(&self, progress: VersionDownloadProgress);
    fn is_cancelled(&self) -> bool;
}

#[async_trait]
pub trait VersionPort: Send + Sync {
    async fn list_installed(&self) -> Result<Vec<InstalledCoreVersion>, PortError>;
    async fn latest(&self, channel: CoreReleaseChannel) -> Result<CoreRelease, PortError>;
    async fn list_releases(&self, limit: usize) -> Result<Vec<CoreReleaseSummary>, PortError>;
    async fn install(
        &self,
        version: String,
        progress: Arc<dyn VersionProgressSink>,
    ) -> Result<(), PortError>;
    async fn activate(&self, version: &str) -> Result<(), PortError>;
    async fn uninstall(&self, version: &str) -> Result<(), PortError>;

    /// Select the previously active, locally installed core version.
    async fn rollback(&self) -> Result<String, PortError> {
        Err(PortError::Failed(
            "core version rollback is not implemented by this host".to_owned(),
        ))
    }

    /// Read the local version-selection journal without network access.
    async fn rollback_snapshot(&self) -> Result<CoreRollbackSnapshot, PortError> {
        Ok(CoreRollbackSnapshot::default())
    }

    /// Latest install-integrity result. Lightweight adapters that do not
    /// retain installation state safely report `Unknown`.
    fn verification(&self) -> CoreArtifactVerification {
        CoreArtifactVerification::Unknown
    }
}
