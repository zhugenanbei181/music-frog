//! Isolated core-version port fixture.
//!
//! The fixture publishes a real installed-version list, a real release
//! catalogue and a real local rollback journal. Every refusal is a typed
//! failure: an unknown release channel is `Unsupported`, a rollback without a
//! previous installed version fails, and the active version cannot be
//! uninstalled.

use async_trait::async_trait;
use infiltrator_contract::version::{
    CoreArtifactVerification, CoreRelease, CoreReleaseSummary, CoreRollbackSnapshot,
    EngineNeutralReleaseChannel, EngineReleaseChannel, InstalledCoreVersion,
    VersionDownloadProgress,
};
use infiltrator_ports::version::{
    VersionActivationRequest, VersionInstallRequest, VersionPort, VersionProgressSink,
    VersionRollbackRequest, VersionUninstallRequest,
};
use lurker_core_contract::capability::Capability;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::EngineId;
use lurker_core_ports::error::PortError;
use std::sync::Mutex;

/// The engine the fixture owns.
pub const CAPTURE_ENGINE: &str = "mihomo";
/// The version the fixture reports as active.
pub const CAPTURE_ACTIVE_VERSION: &str = "v1.19.30";
/// The version the local journal can roll back to.
pub const CAPTURE_ROLLBACK_VERSION: &str = "v1.19.29";
/// The version published on the alpha channel.
pub const CAPTURE_ALPHA_VERSION: &str = "v1.19.31-alpha.1";

/// What the download sink observed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VersionCaptureProgress {
    pub downloaded: u64,
    pub total: Option<u64>,
    pub cancelled: bool,
}

/// A progress sink a capture can observe and cancel.
#[derive(Default)]
pub struct VersionCaptureSink {
    state: Mutex<VersionCaptureProgress>,
}

impl VersionCaptureSink {
    pub fn new() -> Self {
        Self::default()
    }

    /// Requests cancellation; the next install refuses.
    pub fn cancel(&self) {
        self.state.lock().expect("version capture sink").cancelled = true;
    }

    /// The progress the sink observed so far.
    pub fn observed_progress(&self) -> VersionCaptureProgress {
        *self.state.lock().expect("version capture sink")
    }
}

impl VersionProgressSink for VersionCaptureSink {
    fn progress(&self, progress: VersionDownloadProgress) {
        let mut state = self.state.lock().expect("version capture sink");
        state.downloaded = progress.downloaded;
        state.total = progress.total;
    }

    fn is_cancelled(&self) -> bool {
        self.state.lock().expect("version capture sink").cancelled
    }
}

/// An isolated core-version port that answers with a real catalogue.
pub struct VersionCapturePort {
    engine_id: EngineId,
    installed: Mutex<Vec<InstalledCoreVersion>>,
    history: Mutex<Vec<String>>,
    rollback_failure: Mutex<Option<String>>,
    verification: Mutex<CoreArtifactVerification>,
}

impl Default for VersionCapturePort {
    fn default() -> Self {
        Self::new(CAPTURE_ENGINE)
    }
}

impl VersionCapturePort {
    pub fn new(engine: &str) -> Self {
        let engine_id = EngineId::new(engine).expect("valid capture engine ID");
        let installed = vec![
            InstalledCoreVersion {
                engine_id: engine_id.clone(),
                version: CAPTURE_ACTIVE_VERSION.to_owned(),
                path: format!("/isolated-cores/{engine}/{}", CAPTURE_ACTIVE_VERSION),
                is_default: true,
            },
            InstalledCoreVersion {
                engine_id: engine_id.clone(),
                version: CAPTURE_ROLLBACK_VERSION.to_owned(),
                path: format!("/isolated-cores/{engine}/{}", CAPTURE_ROLLBACK_VERSION),
                is_default: false,
            },
        ];
        Self {
            engine_id,
            installed: Mutex::new(installed),
            history: Mutex::new(vec![CAPTURE_ROLLBACK_VERSION.to_owned()]),
            rollback_failure: Mutex::new(None),
            verification: Mutex::new(CoreArtifactVerification::Unknown),
        }
    }

    /// Makes the next rollback fail with the given reason.
    pub fn fail_rollback(&self, reason: impl Into<String>) {
        *self
            .rollback_failure
            .lock()
            .expect("version capture rollback failure") = Some(reason.into());
    }

    /// The installed versions the port currently publishes.
    pub fn installed_versions(&self) -> Vec<InstalledCoreVersion> {
        self.installed
            .lock()
            .expect("version capture installed versions")
            .clone()
    }

    /// The local rollback journal the port publishes.
    pub fn rollback_history(&self) -> Vec<String> {
        self.history
            .lock()
            .expect("version capture history")
            .clone()
    }

    /// The install-integrity fact the port publishes.
    pub fn artifact_verification(&self) -> CoreArtifactVerification {
        self.verification
            .lock()
            .expect("version capture verification")
            .clone()
    }

    fn require_owner_engine(&self, engine_id: &EngineId) -> Result<(), PortError> {
        if engine_id == &self.engine_id {
            return Ok(());
        }
        Err(PortError::Rejected(Failure::new(
            ErrorCode::InvalidState,
            format!(
                "capture version port owns engine {} and cannot serve {}",
                self.engine_id.as_str(),
                engine_id.as_str()
            ),
            false,
        )))
    }

    fn release(&self, version: &str) -> CoreRelease {
        CoreRelease {
            engine_id: self.engine_id.clone(),
            version: version.to_owned(),
            release_date: "2026-10-20".to_owned(),
        }
    }
}

#[async_trait]
impl VersionPort for VersionCapturePort {
    fn engine_id(&self) -> EngineId {
        self.engine_id.clone()
    }

    async fn list_installed(
        &self,
        engine_id: &EngineId,
    ) -> Result<Vec<InstalledCoreVersion>, PortError> {
        self.require_owner_engine(engine_id)?;
        Ok(self.installed_versions())
    }

    async fn release_channels(
        &self,
        engine_id: &EngineId,
    ) -> Result<Vec<EngineReleaseChannel>, PortError> {
        self.require_owner_engine(engine_id)?;
        Ok(EngineNeutralReleaseChannel::ALL
            .into_iter()
            .map(|channel| EngineReleaseChannel::common(self.engine_id.clone(), channel))
            .collect())
    }

    async fn latest(&self, channel: EngineReleaseChannel) -> Result<CoreRelease, PortError> {
        self.require_owner_engine(&channel.engine_id)?;
        match channel.channel_id.as_str() {
            "stable" => Ok(self.release(CAPTURE_ACTIVE_VERSION)),
            "alpha" => Ok(self.release(CAPTURE_ALPHA_VERSION)),
            other => Err(PortError::unsupported(
                Capability::CoreVersionInstall,
                format!("release channel {other} is not published by this capture port"),
            )),
        }
    }

    async fn list_releases(
        &self,
        engine_id: &EngineId,
        limit: usize,
    ) -> Result<Vec<CoreReleaseSummary>, PortError> {
        self.require_owner_engine(engine_id)?;
        let releases = [
            (CAPTURE_ALPHA_VERSION, "v1.19.31-alpha.1", true),
            (CAPTURE_ACTIVE_VERSION, "v1.19.30", false),
            (CAPTURE_ROLLBACK_VERSION, "v1.19.29", false),
        ];
        Ok(releases
            .into_iter()
            .take(limit)
            .map(|(version, name, prerelease)| CoreReleaseSummary {
                engine_id: self.engine_id.clone(),
                version: version.to_owned(),
                name: name.to_owned(),
                published_at: "2026-10-20".to_owned(),
                prerelease,
            })
            .collect())
    }

    async fn install(&self, request: VersionInstallRequest) -> Result<(), PortError> {
        self.require_owner_engine(&request.engine_id)?;
        if request.progress.is_cancelled() {
            *self
                .verification
                .lock()
                .expect("version capture verification") = CoreArtifactVerification::Rejected {
                engine_id: self.engine_id.clone(),
                version: request.version.clone(),
                failure: Failure::new(
                    ErrorCode::Canceled,
                    "core version install was cancelled",
                    false,
                ),
            };
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::Canceled,
                "core version install was cancelled",
                false,
            )));
        }
        let mut installed = self
            .installed
            .lock()
            .expect("version capture installed versions");
        if !installed
            .iter()
            .any(|entry| entry.version == request.version)
        {
            installed.push(InstalledCoreVersion {
                engine_id: request.engine_id.clone(),
                version: request.version.clone(),
                path: format!(
                    "/isolated-cores/{}/{}",
                    request.engine_id.as_str(),
                    request.version
                ),
                is_default: false,
            });
        }
        drop(installed);
        request.progress.progress(VersionDownloadProgress {
            downloaded: 1,
            total: Some(1),
        });
        *self
            .verification
            .lock()
            .expect("version capture verification") = CoreArtifactVerification::Verified {
            engine_id: request.engine_id.clone(),
            version: request.version.clone(),
        };
        Ok(())
    }

    async fn activate(&self, request: VersionActivationRequest) -> Result<(), PortError> {
        self.require_owner_engine(&request.engine_id)?;
        let mut installed = self
            .installed
            .lock()
            .expect("version capture installed versions");
        let Some(entry) = installed
            .iter_mut()
            .find(|entry| entry.version == request.version)
        else {
            return Err(PortError::NotFound(format!(
                "core version {} is not installed",
                request.version
            )));
        };
        entry.is_default = true;
        for other in installed.iter_mut() {
            if other.version != request.version {
                other.is_default = false;
            }
        }
        Ok(())
    }

    async fn uninstall(&self, request: VersionUninstallRequest) -> Result<(), PortError> {
        self.require_owner_engine(&request.engine_id)?;
        let mut installed = self
            .installed
            .lock()
            .expect("version capture installed versions");
        let Some(index) = installed
            .iter()
            .position(|entry| entry.version == request.version)
        else {
            return Err(PortError::NotFound(format!(
                "core version {} is not installed",
                request.version
            )));
        };
        if installed[index].is_default {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::InvalidState,
                format!(
                    "core version {} is active and cannot be uninstalled",
                    request.version
                ),
                false,
            )));
        }
        installed.remove(index);
        Ok(())
    }

    async fn rollback(&self, request: VersionRollbackRequest) -> Result<String, PortError> {
        self.require_owner_engine(&request.engine_id)?;
        if let Some(reason) = self
            .rollback_failure
            .lock()
            .expect("version capture rollback failure")
            .clone()
        {
            return Err(PortError::Failed(reason));
        }
        let installed = self
            .installed
            .lock()
            .expect("version capture installed versions");
        let target = self
            .history
            .lock()
            .expect("version capture history")
            .iter()
            .find(|version| {
                installed
                    .iter()
                    .any(|entry| entry.version == **version && !entry.is_default)
            })
            .cloned();
        match target {
            Some(version) => Ok(version),
            None => Err(PortError::Failed(
                "no previous core version is installed and can be selected".to_owned(),
            )),
        }
    }

    async fn rollback_snapshot(
        &self,
        engine_id: &EngineId,
    ) -> Result<CoreRollbackSnapshot, PortError> {
        self.require_owner_engine(engine_id)?;
        let installed = self
            .installed
            .lock()
            .expect("version capture installed versions");
        let current = installed
            .iter()
            .find(|entry| entry.is_default)
            .map(|entry| entry.version.clone());
        let history = self
            .history
            .lock()
            .expect("version capture history")
            .clone();
        let target = history
            .iter()
            .find(|version| {
                installed
                    .iter()
                    .any(|entry| &entry.version == *version && !entry.is_default)
            })
            .cloned();
        Ok(CoreRollbackSnapshot {
            engine_id: self.engine_id.clone(),
            current,
            target,
            history,
        })
    }

    fn verification(&self, engine_id: &EngineId) -> CoreArtifactVerification {
        if engine_id != &self.engine_id {
            return CoreArtifactVerification::Unknown;
        }
        self.artifact_verification()
    }
}
