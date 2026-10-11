//! Runtime-neutral core-version delivery port.

use crate::error::PortError;
use async_trait::async_trait;
use infiltrator_contract::core_identity::EngineId;
use infiltrator_contract::version::{
    CoreArtifactVerification, CoreRelease, CoreReleaseSummary, CoreRollbackSnapshot,
    EngineNeutralReleaseChannel, EngineReleaseChannel, InstalledCoreVersion,
    VersionDownloadProgress,
};
use std::sync::Arc;

pub trait VersionProgressSink: Send + Sync {
    fn progress(&self, progress: VersionDownloadProgress);
    fn is_cancelled(&self) -> bool;
}

pub struct VersionInstallRequest {
    pub engine_id: EngineId,
    pub version: String,
    pub progress: Arc<dyn VersionProgressSink>,
}

pub struct VersionActivationRequest {
    pub engine_id: EngineId,
    pub version: String,
}

pub struct VersionUninstallRequest {
    pub engine_id: EngineId,
    pub version: String,
}

pub struct VersionRollbackRequest {
    pub engine_id: EngineId,
}

#[async_trait]
pub trait VersionPort: Send + Sync {
    fn engine_id(&self) -> EngineId;
    async fn list_installed(
        &self,
        engine_id: &EngineId,
    ) -> Result<Vec<InstalledCoreVersion>, PortError>;
    async fn release_channels(
        &self,
        engine_id: &EngineId,
    ) -> Result<Vec<EngineReleaseChannel>, PortError> {
        Ok(EngineNeutralReleaseChannel::ALL
            .into_iter()
            .map(|channel| EngineReleaseChannel::common(engine_id.clone(), channel))
            .collect())
    }
    async fn latest(&self, channel: EngineReleaseChannel) -> Result<CoreRelease, PortError>;
    async fn list_releases(
        &self,
        engine_id: &EngineId,
        limit: usize,
    ) -> Result<Vec<CoreReleaseSummary>, PortError>;
    async fn install(&self, request: VersionInstallRequest) -> Result<(), PortError>;
    async fn activate(&self, request: VersionActivationRequest) -> Result<(), PortError>;
    async fn uninstall(&self, request: VersionUninstallRequest) -> Result<(), PortError>;

    /// Select the previously active, locally installed core version.
    async fn rollback(&self, request: VersionRollbackRequest) -> Result<String, PortError> {
        let _ = request;
        Err(PortError::Failed(
            "core version rollback is not implemented by this host".to_owned(),
        ))
    }

    /// Read the local version-selection journal without network access.
    async fn rollback_snapshot(
        &self,
        engine_id: &EngineId,
    ) -> Result<CoreRollbackSnapshot, PortError> {
        Ok(CoreRollbackSnapshot::for_engine(engine_id.clone()))
    }

    /// Latest install-integrity result. Lightweight adapters that do not
    /// retain installation state safely report `Unknown`.
    fn verification(&self, _engine_id: &EngineId) -> CoreArtifactVerification {
        CoreArtifactVerification::Unknown
    }
}
