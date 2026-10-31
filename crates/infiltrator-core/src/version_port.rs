//! Adapter from the concrete mihomo-version manager to the version port.

use crate::error::CoreError;
use infiltrator_contract::version::{
    CoreArtifactVerification, CoreRelease, CoreReleaseSummary, CoreRollbackSnapshot,
    EngineNeutralReleaseChannel, EngineReleaseChannel, InstalledCoreVersion, ReleaseChannelId,
    VersionDownloadProgress,
};
use infiltrator_platform::paths::get_home_dir;
use infiltrator_ports::version::{
    VersionActivationRequest, VersionInstallRequest, VersionPort, VersionRollbackRequest,
    VersionUninstallRequest,
};
use lurker_core_contract::capability::Capability;
use lurker_core_contract::error::{ErrorCode, Failure, FailureReason};
use lurker_core_contract::identity::EngineId;
use lurker_core_ports::error::PortError;
use mihomo_driver::error::MihomoError;
use mihomo_driver::version::engine::engine_id;
use mihomo_version::channel::{Channel, fetch_latest, fetch_releases};
use mihomo_version::download::DownloadProgress;
use mihomo_version::manager::VersionManager;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub struct MihomoVersionPort {
    engine_id: EngineId,
    manager: VersionManager,
    verification: Arc<Mutex<CoreArtifactVerification>>,
}

impl MihomoVersionPort {
    pub fn engine_identifier() -> EngineId {
        engine_id()
    }

    pub fn new(engine_id: EngineId, manager: VersionManager) -> Result<Self, PortError> {
        if engine_id != Self::engine_identifier() || manager.engine_id() != &engine_id {
            return Err(unsupported_engine(&engine_id));
        }
        Ok(Self {
            engine_id,
            manager,
            verification: Arc::new(Mutex::new(CoreArtifactVerification::Unknown)),
        })
    }

    pub fn with_shared_verification(
        engine_id: EngineId,
        manager: VersionManager,
        verification: Arc<Mutex<CoreArtifactVerification>>,
    ) -> Result<Self, PortError> {
        if engine_id != Self::engine_identifier() || manager.engine_id() != &engine_id {
            return Err(unsupported_engine(&engine_id));
        }
        Ok(Self {
            engine_id,
            manager,
            verification,
        })
    }

    pub fn verification_handle(&self) -> Arc<Mutex<CoreArtifactVerification>> {
        Arc::clone(&self.verification)
    }

    pub fn current(engine_id: EngineId) -> Result<Self, CoreError> {
        let manager = VersionManager::for_engine(get_home_dir()?, engine_id.clone())?;
        Self::new(engine_id, manager).map_err(CoreError::from)
    }

    pub fn with_home(home: PathBuf, engine_id: EngineId) -> Result<Self, CoreError> {
        let manager = VersionManager::for_engine(home, engine_id.clone())?;
        Self::new(engine_id, manager).map_err(CoreError::from)
    }
}

#[async_trait::async_trait]
impl VersionPort for MihomoVersionPort {
    fn engine_id(&self) -> EngineId {
        self.engine_id.clone()
    }

    async fn list_installed(
        &self,
        engine_id: &EngineId,
    ) -> Result<Vec<InstalledCoreVersion>, PortError> {
        ensure_mihomo_engine(engine_id)?;
        self.manager
            .list_installed()
            .await
            .map(|versions| {
                versions
                    .into_iter()
                    .map(|version| InstalledCoreVersion {
                        engine_id: self.engine_id.clone(),
                        version: version.version,
                        path: version.path.to_string_lossy().into_owned(),
                        is_default: version.is_default,
                    })
                    .collect()
            })
            .map_err(version_error)
    }

    async fn release_channels(
        &self,
        engine_id: &EngineId,
    ) -> Result<Vec<EngineReleaseChannel>, PortError> {
        ensure_mihomo_engine(engine_id)?;
        Ok(vec![
            EngineReleaseChannel::common(
                self.engine_id.clone(),
                EngineNeutralReleaseChannel::Stable,
            ),
            EngineReleaseChannel::common(
                self.engine_id.clone(),
                EngineNeutralReleaseChannel::Alpha,
            ),
            EngineReleaseChannel::new(
                self.engine_id.clone(),
                ReleaseChannelId::new("meta-core")
                    .expect("Mihomo adapter release channel ID is valid"),
            ),
        ])
    }

    async fn latest(&self, channel: EngineReleaseChannel) -> Result<CoreRelease, PortError> {
        ensure_mihomo_engine(&channel.engine_id)?;
        if channel.engine_id != self.engine_id {
            return Err(unsupported_engine(&channel.engine_id));
        }
        let channel = to_channel(&channel.channel_id)?;
        fetch_latest(channel)
            .await
            .map(|release| CoreRelease {
                engine_id: self.engine_id.clone(),
                version: release.version,
                release_date: release.release_date,
            })
            .map_err(version_error)
    }

    async fn list_releases(
        &self,
        engine_id: &EngineId,
        limit: usize,
    ) -> Result<Vec<CoreReleaseSummary>, PortError> {
        ensure_mihomo_engine(engine_id)?;
        fetch_releases(limit)
            .await
            .map(|releases| {
                releases
                    .into_iter()
                    .map(|release| CoreReleaseSummary {
                        engine_id: self.engine_id.clone(),
                        version: release.version,
                        name: release.name,
                        published_at: release.published_at,
                        prerelease: release.prerelease,
                    })
                    .collect()
            })
            .map_err(version_error)
    }

    async fn install(&self, request: VersionInstallRequest) -> Result<(), PortError> {
        ensure_mihomo_engine(&request.engine_id)?;
        let VersionInstallRequest {
            version, progress, ..
        } = request;
        let result = self
            .manager
            .install_with_progress_and_cancel(
                &version,
                |item| progress.progress(to_progress(item)),
                || progress.is_cancelled(),
            )
            .await
            .map_err(version_error);
        let verification = match &result {
            Ok(()) => CoreArtifactVerification::Verified {
                engine_id: self.engine_id.clone(),
                version: version.clone(),
            },
            Err(error) => CoreArtifactVerification::Rejected {
                engine_id: self.engine_id.clone(),
                version,
                failure: Failure::from(error.clone()),
            },
        };
        *self.verification.lock().expect("version verification lock") = verification;
        result
    }

    async fn activate(&self, request: VersionActivationRequest) -> Result<(), PortError> {
        ensure_mihomo_engine(&request.engine_id)?;
        self.manager
            .set_default(&request.version)
            .await
            .map_err(version_error)
    }

    async fn uninstall(&self, request: VersionUninstallRequest) -> Result<(), PortError> {
        ensure_mihomo_engine(&request.engine_id)?;
        self.manager
            .uninstall(&request.version)
            .await
            .map_err(version_error)
    }

    async fn rollback(&self, request: VersionRollbackRequest) -> Result<String, PortError> {
        ensure_mihomo_engine(&request.engine_id)?;
        self.manager.rollback().await.map_err(version_error)
    }

    async fn rollback_snapshot(
        &self,
        engine_id: &EngineId,
    ) -> Result<CoreRollbackSnapshot, PortError> {
        ensure_mihomo_engine(engine_id)?;
        self.manager
            .rollback_info()
            .await
            .map(|info| CoreRollbackSnapshot {
                engine_id: self.engine_id.clone(),
                current: info.current,
                target: info.target,
                history: info.history,
            })
            .map_err(version_error)
    }

    fn verification(&self, engine_id: &EngineId) -> CoreArtifactVerification {
        if ensure_mihomo_engine(engine_id).is_err() || engine_id != &self.engine_id {
            return CoreArtifactVerification::Unknown;
        }
        match self
            .verification
            .lock()
            .expect("version verification lock")
            .clone()
        {
            CoreArtifactVerification::Verified {
                engine_id: result_engine,
                version,
            } if &result_engine == engine_id => CoreArtifactVerification::Verified {
                engine_id: result_engine,
                version,
            },
            CoreArtifactVerification::Rejected {
                engine_id: result_engine,
                version,
                failure,
            } if &result_engine == engine_id => CoreArtifactVerification::Rejected {
                engine_id: result_engine,
                version,
                failure,
            },
            CoreArtifactVerification::Unknown
            | CoreArtifactVerification::Verified { .. }
            | CoreArtifactVerification::Rejected { .. } => CoreArtifactVerification::Unknown,
        }
    }
}

fn to_progress(progress: DownloadProgress) -> VersionDownloadProgress {
    VersionDownloadProgress {
        downloaded: progress.downloaded,
        total: progress.total,
    }
}

fn unsupported_engine(engine_id: &EngineId) -> PortError {
    PortError::unsupported(
        Capability::CoreVersionInstall,
        format!(
            "Mihomo version adapter cannot serve engine {}",
            engine_id.as_str()
        ),
    )
}

fn to_channel(channel: &ReleaseChannelId) -> Result<Channel, PortError> {
    match channel {
        channel if channel.as_str() == "stable" => Ok(Channel::Stable),
        channel if channel.as_str() == "alpha" => Ok(Channel::Alpha),
        channel if channel.as_str() == "meta-core" => Ok(Channel::MetaCore),
        _ => Err(PortError::unsupported(
            Capability::CoreVersionInstall,
            format!("Mihomo release channel {} is unsupported", channel.as_str()),
        )),
    }
}

fn ensure_mihomo_engine(engine_id: &EngineId) -> Result<(), PortError> {
    if engine_id == &MihomoVersionPort::engine_identifier() {
        Ok(())
    } else {
        Err(unsupported_engine(engine_id))
    }
}

fn version_error(error: MihomoError) -> PortError {
    if matches!(error, MihomoError::Canceled) {
        return PortError::Rejected(
            Failure::new(ErrorCode::Canceled, error.to_string(), false)
                .with_reason(FailureReason::DownloadCanceled),
        );
    }
    match error {
        MihomoError::Http(error) => PortError::Network(error.to_string()),
        MihomoError::Io(error) => PortError::Io(error.to_string()),
        other => PortError::Failed(other.to_string()),
    }
}

#[cfg(test)]
#[path = "version_port_tests.rs"]
mod tests;
