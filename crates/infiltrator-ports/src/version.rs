//! Runtime-neutral core-version delivery port.

use async_trait::async_trait;
use infiltrator_contract::version::{
    CoreRelease, CoreReleaseChannel, CoreReleaseSummary, InstalledCoreVersion,
    VersionDownloadProgress,
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
}
