//! Core-version use-cases over a host-provided version port.

use infiltrator_contract::error::Failure;
use infiltrator_contract::version::{
    CoreRelease, CoreReleaseChannel, CoreReleaseSummary, InstalledCoreVersion,
    VersionDownloadProgress,
};
use infiltrator_ports::version::{VersionPort, VersionProgressSink};
use std::sync::Arc;

#[derive(Clone)]
pub struct VersionApplication {
    port: Arc<dyn VersionPort>,
}

impl VersionApplication {
    pub fn new(port: Arc<dyn VersionPort>) -> Self {
        Self { port }
    }

    pub async fn list_installed(&self) -> Result<Vec<InstalledCoreVersion>, Failure> {
        self.port.list_installed().await.map_err(Failure::from)
    }

    pub async fn latest(&self, channel: CoreReleaseChannel) -> Result<CoreRelease, Failure> {
        self.port.latest(channel).await.map_err(Failure::from)
    }

    pub async fn list_releases(&self, limit: usize) -> Result<Vec<CoreReleaseSummary>, Failure> {
        self.port.list_releases(limit).await.map_err(Failure::from)
    }

    pub async fn install(
        &self,
        version: String,
        progress: Arc<dyn VersionProgressSink>,
    ) -> Result<(), Failure> {
        self.port
            .install(version, progress)
            .await
            .map_err(Failure::from)
    }

    pub async fn activate(&self, version: &str) -> Result<(), Failure> {
        self.port.activate(version).await.map_err(Failure::from)
    }

    pub async fn uninstall(&self, version: &str) -> Result<(), Failure> {
        self.port.uninstall(version).await.map_err(Failure::from)
    }
}

/// No-op progress sink for one-shot CLI/API calls that do not render a live
/// progress stream.
#[derive(Default)]
pub struct QuietVersionProgress;

impl VersionProgressSink for QuietVersionProgress {
    fn progress(&self, _progress: VersionDownloadProgress) {}

    fn is_cancelled(&self) -> bool {
        false
    }
}
