mod asset;
mod install;
mod state;

use asset::LockedSingBoxRelease;
use install::{
    HttpSingBoxArchiveDownloader, InstallPlan, InstallStage, ProcessSingBoxBinaryInspector,
    PublishedInstall, SingBoxArchiveDownloader, SingBoxBinaryInspector,
};
use state::{
    InstalledSingBoxReceipt, VerifiedInstalledBinary, VersionSelectionJournal,
    VersionSelectionState, unique_path, write_json_atomically,
};

use async_trait::async_trait;
use infiltrator_contract::capability::Capability;
use infiltrator_contract::core_capabilities::EngineBinaryTarget;
use infiltrator_contract::core_identity::{CoreVersion, EngineId};
use infiltrator_contract::error::{ErrorCode, Failure, FailureReason};
use infiltrator_contract::version::{
    CoreArtifactVerification, CoreRelease, CoreReleaseSummary, CoreRollbackSnapshot,
    EngineNeutralReleaseChannel, EngineReleaseChannel, InstalledCoreVersion,
};
use infiltrator_ports::core_binary_selection::{
    CoreBinarySelection, CoreBinarySelectionPolicy, CoreBinarySelectionPort,
    CoreBinarySelectionRequest, CoreBinarySelectionSource,
};
use infiltrator_ports::engine_version_backend_factory::EngineVersionBackendFactory;
use infiltrator_ports::error::PortError;
use infiltrator_ports::version::{
    VersionActivationRequest, VersionInstallRequest, VersionPort, VersionProgressSink,
    VersionRollbackRequest, VersionUninstallRequest,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::io::{Error, ErrorKind};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::fs;
use tokio::sync;

const EMBEDDED_ASSET_LOCK: &str = include_str!("../../../packaging/sing-box-evidence-assets.json");
const VERSION_STATE_SCHEMA: u32 = 1;
const VERSION_HISTORY_LIMIT: usize = 8;
const MAX_ARCHIVE_BYTES: u64 = 256 * 1024 * 1024;

fn engine_id() -> EngineId {
    EngineId::sing_box()
}

/// Exact Cargo binary target represented by an asset-lock record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SingBoxBinaryTarget {
    LinuxX86_64Gnu,
    LinuxAarch64Gnu,
    AndroidAarch64,
    AndroidX86_64,
}

impl SingBoxBinaryTarget {
    pub fn from_triple(triple: &str) -> Option<Self> {
        match triple {
            "x86_64-unknown-linux-gnu" => Some(Self::LinuxX86_64Gnu),
            "aarch64-unknown-linux-gnu" => Some(Self::LinuxAarch64Gnu),
            "aarch64-linux-android" => Some(Self::AndroidAarch64),
            "x86_64-linux-android" => Some(Self::AndroidX86_64),
            _ => None,
        }
    }

    fn current() -> Result<Self, PortError> {
        #[cfg(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu"))]
        {
            return Ok(Self::LinuxX86_64Gnu);
        }
        #[cfg(all(target_os = "linux", target_arch = "aarch64", target_env = "gnu"))]
        {
            return Ok(Self::LinuxAarch64Gnu);
        }
        #[cfg(all(target_os = "android", target_arch = "aarch64"))]
        {
            return Ok(Self::AndroidAarch64);
        }
        #[cfg(all(target_os = "android", target_arch = "x86_64"))]
        {
            return Ok(Self::AndroidX86_64);
        }
        #[allow(unreachable_code)]
        Err(unsupported_target(
            "the current Cargo target is not in the sing-box asset lock",
        ))
    }

    pub fn as_triple(self) -> &'static str {
        match self {
            Self::LinuxX86_64Gnu => "x86_64-unknown-linux-gnu",
            Self::LinuxAarch64Gnu => "aarch64-unknown-linux-gnu",
            Self::AndroidAarch64 => "aarch64-linux-android",
            Self::AndroidX86_64 => "x86_64-linux-android",
        }
    }
}

/// Production version and binary-selection adapters for one exact target.
pub struct SingBoxEngineVersionBackendFactory {
    backend: Arc<SingBoxVersionBackend>,
}

impl SingBoxEngineVersionBackendFactory {
    /// Resolve the current build target and the repository's embedded immutable
    /// asset lock. Unsupported targets fail before any directory is created.
    pub fn current(home: PathBuf) -> Result<Self, PortError> {
        let target = SingBoxBinaryTarget::current()?;
        let release = Arc::new(LockedSingBoxRelease::embedded()?);
        let _ = release.asset(target, &release.version)?;
        Ok(Self {
            backend: Arc::new(SingBoxVersionBackend::new(
                home.join("engines").join(engine_id().as_str()),
                target,
                release,
                Arc::new(HttpSingBoxArchiveDownloader),
                Arc::new(ProcessSingBoxBinaryInspector),
            )),
        })
    }
}

impl EngineVersionBackendFactory for SingBoxEngineVersionBackendFactory {
    fn version_port(&self, engine: &EngineId) -> Result<Arc<dyn VersionPort>, PortError> {
        ensure_engine(engine)?;
        Ok(self.backend.clone())
    }

    fn binary_selection_port(
        &self,
        engine: &EngineId,
    ) -> Result<Arc<dyn CoreBinarySelectionPort>, PortError> {
        ensure_engine(engine)?;
        Ok(self.backend.clone())
    }
}

struct SingBoxVersionBackend {
    engine_data_dir: PathBuf,
    target: SingBoxBinaryTarget,
    release: Arc<LockedSingBoxRelease>,
    downloader: Arc<dyn SingBoxArchiveDownloader>,
    inspector: Arc<dyn SingBoxBinaryInspector>,
    operation_lock: sync::Mutex<()>,
    verification: Mutex<CoreArtifactVerification>,
}

impl SingBoxVersionBackend {
    fn new(
        engine_data_dir: PathBuf,
        target: SingBoxBinaryTarget,
        release: Arc<LockedSingBoxRelease>,
        downloader: Arc<dyn SingBoxArchiveDownloader>,
        inspector: Arc<dyn SingBoxBinaryInspector>,
    ) -> Self {
        Self {
            engine_data_dir,
            target,
            release,
            downloader,
            inspector,
            operation_lock: sync::Mutex::new(()),
            verification: Mutex::new(CoreArtifactVerification::Unknown),
        }
    }

    fn versions_dir(&self) -> PathBuf {
        self.target_dir().join("versions")
    }

    fn target_dir(&self) -> PathBuf {
        self.engine_data_dir
            .join("targets")
            .join(self.target.as_triple())
    }

    fn version_dir(&self, version: &str) -> Result<PathBuf, PortError> {
        validate_version(version)?;
        Ok(self.versions_dir().join(version))
    }

    async fn read_selection(&self) -> Result<VersionSelectionJournal, PortError> {
        let path = self.target_dir().join("version-selection.json");
        match fs::read(&path).await {
            Ok(bytes) => {
                let journal: VersionSelectionJournal =
                    serde_json::from_slice(&bytes).map_err(|error| {
                        invalid_journal(format!("invalid version selection: {error}"))
                    })?;
                journal.validate(self.target, &self.release)?;
                Ok(journal)
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {
                Ok(VersionSelectionJournal::empty(self.target))
            }
            Err(error) => Err(PortError::Io(error.to_string())),
        }
    }

    async fn verify_installed(&self, version: &str) -> Result<VerifiedInstalledBinary, PortError> {
        let asset = self.release.asset(self.target, version)?;
        let directory = self.version_dir(version)?;
        let binary_path = directory.join("sing-box");
        let receipt_path = directory.join("install.json");
        let receipt_bytes = fs::read(&receipt_path)
            .await
            .map_err(|error| not_found_or_io(error, "sing-box installation receipt"))?;
        let receipt: InstalledSingBoxReceipt =
            serde_json::from_slice(&receipt_bytes).map_err(|error| {
                invalid_installation(format!("invalid sing-box installation receipt: {error}"))
            })?;
        if receipt.schema_version != 1
            || receipt.engine_id != engine_id().as_str()
            || receipt.version != asset.version
            || receipt.target != asset.target
            || receipt.adapter_revision != asset.adapter_revision
            || receipt.archive != asset.archive
            || receipt.archive_sha256 != asset.archive_sha256
            || !valid_digest(&receipt.binary_sha256)
        {
            return Err(invalid_installation(
                "sing-box receipt does not match the locked engine, target, version, and adapter revision",
            ));
        }
        let binary_bytes = fs::read(&binary_path)
            .await
            .map_err(|error| not_found_or_io(error, "sing-box executable"))?;
        if sha256_hex(&binary_bytes) != receipt.binary_sha256 {
            return Err(invalid_installation(
                "installed sing-box executable digest differs from its verified receipt",
            ));
        }
        let reported = self.inspector.version(&binary_path).await?;
        if reported.raw() != asset.version {
            return Err(invalid_installation(format!(
                "locked sing-box version {} differs from binary-reported {}",
                asset.version,
                reported.raw()
            )));
        }
        Ok(VerifiedInstalledBinary {
            version: reported,
            binary_path,
            receipt,
        })
    }

    async fn write_selection(
        &self,
        previous: VersionSelectionJournal,
        state: VersionSelectionState,
    ) -> Result<(), PortError> {
        let revision = previous
            .revision
            .checked_add(1)
            .ok_or_else(|| invalid_journal("sing-box version selection revision overflow"))?;
        let journal = VersionSelectionJournal {
            schema_version: VERSION_STATE_SCHEMA,
            engine_id: engine_id().as_str().to_owned(),
            target: self.target.as_triple().to_owned(),
            adapter_revision: match state.selected_version() {
                Some(version) => self.release.asset(self.target, version)?.adapter_revision,
                None => 0,
            },
            revision,
            state,
        };
        write_json_atomically(&self.target_dir().join("version-selection.json"), &journal).await
    }

    fn record_verification(&self, verification: CoreArtifactVerification) {
        let mut current = self
            .verification
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *current = verification;
    }

    async fn install_locked(
        &self,
        version: String,
        progress: Arc<dyn VersionProgressSink>,
    ) -> Result<(), PortError> {
        let requested_version = version.clone();
        let asset = self.release.asset(self.target, &version)?;
        if progress.is_cancelled() {
            return Err(cancelled_install());
        }

        let plan = InstallPlan { asset };
        let stage = InstallStage::Planned(plan);
        let stage = stage
            .download(self.downloader.as_ref(), progress.as_ref())
            .await?;
        let stage = stage.verify_archive()?;
        let stage = stage.extract_binary()?;
        let stage = stage
            .verify_binary(self.inspector.as_ref(), self.target, &self.versions_dir())
            .await?;
        let stage = stage.stage(self).await?;
        if progress.is_cancelled() {
            return stage.cancel().await;
        }
        let published = match stage.publish(self).await? {
            PublishedInstall::Created { version }
            | PublishedInstall::AlreadyPresent { version } => InstallStage::Published { version },
        };
        match published {
            InstallStage::Published { version } if version == requested_version => Ok(()),
            InstallStage::Published { .. } => Err(invalid_installation(
                "sing-box install published a version other than the requested locked release",
            )),
            InstallStage::Planned(_)
            | InstallStage::Downloaded(_)
            | InstallStage::ArchiveVerified(_)
            | InstallStage::BinaryExtracted(_)
            | InstallStage::BinaryVerified(_)
            | InstallStage::Staged(_) => Err(invalid_installation(
                "sing-box install transaction did not reach publication",
            )),
        }
    }
}

#[async_trait]
impl VersionPort for SingBoxVersionBackend {
    fn engine_id(&self) -> EngineId {
        engine_id()
    }

    async fn list_installed(
        &self,
        requested_engine: &EngineId,
    ) -> Result<Vec<InstalledCoreVersion>, PortError> {
        ensure_engine(requested_engine)?;
        let _guard = self.operation_lock.lock().await;
        let selection = self.read_selection().await?;
        let mut installed = Vec::new();
        let mut versions = BTreeSet::new();
        for asset in self.release.target_assets(self.target) {
            if !versions.insert(asset.version.clone()) {
                continue;
            }
            let directory = self.version_dir(&asset.version)?;
            if !fs::try_exists(&directory)
                .await
                .map_err(|error| PortError::Io(error.to_string()))?
            {
                continue;
            }
            let verified = self.verify_installed(&asset.version).await?;
            installed.push(InstalledCoreVersion {
                engine_id: engine_id(),
                version: verified.version.raw().to_owned(),
                path: verified.binary_path.to_string_lossy().into_owned(),
                is_default: selection.state.selected_version() == Some(verified.version.raw()),
            });
        }
        Ok(installed)
    }

    async fn release_channels(
        &self,
        requested_engine: &EngineId,
    ) -> Result<Vec<EngineReleaseChannel>, PortError> {
        ensure_engine(requested_engine)?;
        self.release.asset(self.target, &self.release.version)?;
        Ok(vec![EngineReleaseChannel::common(
            engine_id(),
            EngineNeutralReleaseChannel::Stable,
        )])
    }

    async fn latest(&self, channel: EngineReleaseChannel) -> Result<CoreRelease, PortError> {
        ensure_engine(&channel.engine_id)?;
        self.release.asset(self.target, &self.release.version)?;
        if channel.channel_id.as_str() != "stable" {
            return Err(PortError::unsupported(
                Capability::CoreVersionInstall,
                format!(
                    "sing-box release channel {} is not locked",
                    channel.channel_id.as_str()
                ),
            ));
        }
        Ok(CoreRelease {
            engine_id: engine_id(),
            version: self.release.version.clone(),
            release_date: self.release.published_at.clone(),
        })
    }

    async fn list_releases(
        &self,
        requested_engine: &EngineId,
        limit: usize,
    ) -> Result<Vec<CoreReleaseSummary>, PortError> {
        ensure_engine(requested_engine)?;
        if limit == 0 {
            return Ok(Vec::new());
        }
        self.release.asset(self.target, &self.release.version)?;
        Ok(vec![CoreReleaseSummary {
            engine_id: engine_id(),
            version: self.release.version.clone(),
            name: format!("sing-box {}", self.release.version),
            published_at: self.release.published_at.clone(),
            prerelease: false,
        }])
    }

    async fn install(&self, request: VersionInstallRequest) -> Result<(), PortError> {
        ensure_engine(&request.engine_id)?;
        let version = request.version;
        let result = {
            let _guard = self.operation_lock.lock().await;
            self.install_locked(version.clone(), request.progress).await
        };
        let verification = match &result {
            Ok(()) => CoreArtifactVerification::Verified {
                engine_id: engine_id(),
                version,
            },
            Err(error) => CoreArtifactVerification::Rejected {
                engine_id: engine_id(),
                version,
                failure: Failure::from(error.clone()),
            },
        };
        self.record_verification(verification);
        result
    }

    async fn activate(&self, request: VersionActivationRequest) -> Result<(), PortError> {
        ensure_engine(&request.engine_id)?;
        let _guard = self.operation_lock.lock().await;
        let verified = self.verify_installed(&request.version).await?;
        let journal = self.read_selection().await?;
        let previous = journal.state.selected_version().map(str::to_owned);
        if previous.as_deref() == Some(verified.version.raw()) {
            return Ok(());
        }
        let mut history = journal.state.history();
        if let Some(current) = previous.as_deref() {
            history.retain(|version| version != current);
            history.insert(0, current.to_owned());
        }
        history.truncate(VERSION_HISTORY_LIMIT);
        self.write_selection(
            journal,
            VersionSelectionState::Selected {
                version: verified.version.raw().to_owned(),
                previous,
                history,
            },
        )
        .await
    }

    async fn uninstall(&self, request: VersionUninstallRequest) -> Result<(), PortError> {
        ensure_engine(&request.engine_id)?;
        let _guard = self.operation_lock.lock().await;
        let version = validate_version(&request.version).map(|_| request.version)?;
        let journal = self.read_selection().await?;
        if journal.state.selected_version() == Some(version.as_str()) {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::InvalidState,
                "cannot uninstall the active sing-box version",
                false,
            )));
        }
        self.release.asset(self.target, &version)?;
        let directory = self.version_dir(&version)?;
        match fs::remove_dir_all(directory).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
            Err(error) => Err(PortError::Io(error.to_string())),
        }
    }

    async fn rollback(&self, request: VersionRollbackRequest) -> Result<String, PortError> {
        ensure_engine(&request.engine_id)?;
        let _guard = self.operation_lock.lock().await;
        let journal = self.read_selection().await?;
        let (current, target, mut history) = match journal.state.clone() {
            VersionSelectionState::Unselected => {
                return Err(PortError::NotFound(
                    "sing-box has no selected version to roll back".to_owned(),
                ));
            }
            VersionSelectionState::Selected {
                version,
                previous: Some(previous),
                history,
            } => (version, previous, history),
            VersionSelectionState::Selected { previous: None, .. } => {
                return Err(PortError::NotFound(
                    "sing-box has no prior installed version to roll back to".to_owned(),
                ));
            }
        };
        self.verify_installed(&target).await?;
        history.retain(|version| version != &current);
        history.insert(0, current.clone());
        history.truncate(VERSION_HISTORY_LIMIT);
        self.write_selection(
            journal,
            VersionSelectionState::Selected {
                version: target.clone(),
                previous: Some(current),
                history,
            },
        )
        .await?;
        Ok(target)
    }

    async fn rollback_snapshot(
        &self,
        requested_engine: &EngineId,
    ) -> Result<CoreRollbackSnapshot, PortError> {
        ensure_engine(requested_engine)?;
        let _guard = self.operation_lock.lock().await;
        let journal = self.read_selection().await?;
        match journal.state {
            VersionSelectionState::Unselected => Ok(CoreRollbackSnapshot::for_engine(engine_id())),
            VersionSelectionState::Selected {
                version,
                previous,
                history,
            } => {
                let target = match previous {
                    Some(previous) => match self.verify_installed(&previous).await {
                        Ok(_) => Some(previous),
                        Err(PortError::NotFound(_)) => None,
                        Err(error) => return Err(error),
                    },
                    None => None,
                };
                Ok(CoreRollbackSnapshot {
                    engine_id: engine_id(),
                    current: Some(version),
                    target,
                    history,
                })
            }
        }
    }

    fn verification(&self, requested_engine: &EngineId) -> CoreArtifactVerification {
        if ensure_engine(requested_engine).is_err() {
            return CoreArtifactVerification::Unknown;
        }
        self.verification
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }
}

#[async_trait]
impl CoreBinarySelectionPort for SingBoxVersionBackend {
    fn engine_id(&self) -> EngineId {
        engine_id()
    }

    async fn select(
        &self,
        request: CoreBinarySelectionRequest,
    ) -> Result<CoreBinarySelection, PortError> {
        ensure_engine(&request.engine_id)?;
        let _guard = self.operation_lock.lock().await;
        match request.policy {
            CoreBinarySelectionPolicy::ManagedOnly => {}
            CoreBinarySelectionPolicy::ManagedWithBundledFallback(_)
            | CoreBinarySelectionPolicy::PreferBundled(_)
            | CoreBinarySelectionPolicy::RequireBundled(_) => {
                return Err(PortError::unsupported(
                    Capability::CoreLifecycle,
                    "sing-box bundled binary policy has no target-bound provenance adapter",
                ));
            }
        }
        let journal = self.read_selection().await?;
        let version = match journal.state {
            VersionSelectionState::Unselected => {
                return Err(PortError::NotFound(
                    "no sing-box version has been explicitly activated".to_owned(),
                ));
            }
            VersionSelectionState::Selected { version, .. } => version,
        };
        let verified = self.verify_installed(&version).await?;
        let binary_target = EngineBinaryTarget::from_target_triple(self.target.as_triple())
            .ok_or_else(|| {
                PortError::unsupported(
                    Capability::CoreLifecycle,
                    "sing-box version backend target is not a registered Core binary target",
                )
            })?;
        Ok(CoreBinarySelection {
            engine_id: engine_id(),
            binary_target,
            version: verified.version,
            binary_path: verified.binary_path,
            related_asset_candidates: Vec::new(),
            source: CoreBinarySelectionSource::ManagedVersion,
            verification: CoreArtifactVerification::Verified {
                engine_id: engine_id(),
                version,
            },
        })
    }
}

pub(super) fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub(super) fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub(super) fn validate_version(version: &str) -> Result<(), PortError> {
    let canonical = version.strip_prefix('v').ok_or_else(|| {
        invalid_installation("sing-box version must use the canonical v<version> form")
    })?;
    let numeric = canonical.split(['-', '+']).next().unwrap_or_default();
    let components: Vec<&str> = numeric.split('.').collect();
    if canonical.is_empty()
        || components.len() != 3
        || components.iter().any(|component| {
            component.is_empty() || !component.bytes().all(|byte| byte.is_ascii_digit())
        })
        || !canonical
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'+'))
    {
        return Err(invalid_installation(
            "sing-box version contains invalid filesystem or identity characters",
        ));
    }
    CoreVersion::new(version.to_owned())
        .map(|_| ())
        .map_err(|error| invalid_installation(error.to_string()))
}

pub(super) fn ensure_engine(engine: &EngineId) -> Result<(), PortError> {
    if engine == &engine_id() {
        Ok(())
    } else {
        Err(unsupported_engine(engine))
    }
}

pub(super) fn unsupported_engine(engine: &EngineId) -> PortError {
    PortError::unsupported(
        Capability::CoreVersionInstall,
        format!(
            "sing-box version backend cannot serve engine {}",
            engine.as_str()
        ),
    )
}

pub(super) fn unsupported_target(reason: &str) -> PortError {
    PortError::unsupported(Capability::CoreVersionInstall, reason)
}

pub(super) fn invalid_lock(reason: impl Into<String>) -> PortError {
    PortError::Rejected(Failure::new(ErrorCode::Configuration, reason, false))
}

pub(super) fn invalid_installation(reason: impl Into<String>) -> PortError {
    PortError::Rejected(Failure::new(ErrorCode::Configuration, reason, false))
}

pub(super) fn invalid_journal(reason: impl Into<String>) -> PortError {
    PortError::Rejected(Failure::new(ErrorCode::Storage, reason, false))
}

pub(super) fn cancelled_install() -> PortError {
    PortError::Rejected(
        Failure::new(ErrorCode::Canceled, "sing-box install was canceled", false)
            .with_reason(FailureReason::DownloadCanceled),
    )
}

pub(super) fn not_found_or_io(error: Error, label: &str) -> PortError {
    if error.kind() == ErrorKind::NotFound {
        PortError::NotFound(format!("{label} is not installed"))
    } else {
        PortError::Io(error.to_string())
    }
}

#[cfg(test)]
#[path = "version_tests.rs"]
mod tests;
