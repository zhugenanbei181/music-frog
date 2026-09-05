//! Adapter from the concrete mihomo-version manager to the version port.

use infiltrator_contract::version::{
    CoreRelease, CoreReleaseChannel, CoreReleaseSummary, InstalledCoreVersion,
    VersionDownloadProgress,
};
use infiltrator_ports::error::PortError;
use infiltrator_ports::version::{VersionPort, VersionProgressSink};
use mihomo_version::channel::{Channel, fetch_latest, fetch_releases};
use mihomo_version::download::DownloadProgress;
use mihomo_version::manager::VersionManager;
use std::path::PathBuf;
use std::sync::Arc;

pub struct MihomoVersionPort {
    manager: VersionManager,
}

impl MihomoVersionPort {
    pub fn new(manager: VersionManager) -> Self {
        Self { manager }
    }

    pub fn current() -> anyhow::Result<Self> {
        Ok(Self::new(VersionManager::new()?))
    }

    pub fn with_home(home: PathBuf) -> anyhow::Result<Self> {
        Ok(Self::new(VersionManager::with_home(home)?))
    }
}

#[async_trait::async_trait]
impl VersionPort for MihomoVersionPort {
    async fn list_installed(&self) -> Result<Vec<InstalledCoreVersion>, PortError> {
        self.manager
            .list_installed()
            .await
            .map(|versions| {
                versions
                    .into_iter()
                    .map(|version| InstalledCoreVersion {
                        version: version.version,
                        path: version.path.to_string_lossy().into_owned(),
                        is_default: version.is_default,
                    })
                    .collect()
            })
            .map_err(version_error)
    }

    async fn latest(&self, channel: CoreReleaseChannel) -> Result<CoreRelease, PortError> {
        fetch_latest(to_channel(channel))
            .await
            .map(|release| CoreRelease {
                version: release.version,
                release_date: release.release_date,
            })
            .map_err(version_error)
    }

    async fn list_releases(&self, limit: usize) -> Result<Vec<CoreReleaseSummary>, PortError> {
        fetch_releases(limit)
            .await
            .map(|releases| {
                releases
                    .into_iter()
                    .map(|release| CoreReleaseSummary {
                        version: release.version,
                        name: release.name,
                        published_at: release.published_at,
                        prerelease: release.prerelease,
                    })
                    .collect()
            })
            .map_err(version_error)
    }

    async fn install(
        &self,
        version: String,
        progress: Arc<dyn VersionProgressSink>,
    ) -> Result<(), PortError> {
        self.manager
            .install_with_progress_and_cancel(
                &version,
                |item| progress.progress(to_progress(item)),
                || progress.is_cancelled(),
            )
            .await
            .map_err(version_error)
    }

    async fn activate(&self, version: &str) -> Result<(), PortError> {
        self.manager.set_default(version).await.map_err(version_error)
    }

    async fn uninstall(&self, version: &str) -> Result<(), PortError> {
        self.manager.uninstall(version).await.map_err(version_error)
    }
}

fn to_progress(progress: DownloadProgress) -> VersionDownloadProgress {
    VersionDownloadProgress {
        downloaded: progress.downloaded,
        total: progress.total,
    }
}

fn to_channel(channel: CoreReleaseChannel) -> Channel {
    match channel {
        CoreReleaseChannel::Stable => Channel::Stable,
        CoreReleaseChannel::Beta => Channel::Beta,
        CoreReleaseChannel::Nightly => Channel::Nightly,
    }
}

fn version_error(error: mihomo_api::error::MihomoError) -> PortError {
    match error {
        mihomo_api::error::MihomoError::Http(error) => PortError::Network(error.to_string()),
        mihomo_api::error::MihomoError::Io(error) => PortError::Io(error.to_string()),
        other => PortError::Failed(other.to_string()),
    }
}
