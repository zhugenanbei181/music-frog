use super::asset::LockedSingBoxAsset;
use super::state::{InstalledSingBoxReceipt, write_json_atomically};
use super::{
    MAX_ARCHIVE_BYTES, SingBoxBinaryTarget, SingBoxVersionBackend, cancelled_install, engine_id,
    invalid_installation, sha256_hex, unique_path, validate_version,
};
#[cfg(not(unix))]
use super::unsupported_target;
use async_trait::async_trait;
use flate2::read::GzDecoder;
use futures_util::StreamExt;
use infiltrator_contract::version::VersionDownloadProgress;
use infiltrator_ports::error::PortError;
use infiltrator_ports::version::VersionProgressSink;
use lurker_core_contract::identity::CoreVersion;
use std::io::ErrorKind;
use std::io::Read;
use std::path::{Path, PathBuf};
use tar::Archive;
use tokio::fs;
use tokio::process::Command;

#[async_trait]
pub(super) trait SingBoxArchiveDownloader: Send + Sync {
    async fn download(
        &self,
        asset: &LockedSingBoxAsset,
        progress: &dyn VersionProgressSink,
    ) -> Result<Vec<u8>, PortError>;
}

pub(super) struct HttpSingBoxArchiveDownloader;

#[async_trait]
impl SingBoxArchiveDownloader for HttpSingBoxArchiveDownloader {
    async fn download(
        &self,
        asset: &LockedSingBoxAsset,
        progress: &dyn VersionProgressSink,
    ) -> Result<Vec<u8>, PortError> {
        if progress.is_cancelled() {
            return Err(cancelled_install());
        }
        let response = reqwest::get(&asset.download_url)
            .await
            .map_err(|error| PortError::Network(error.to_string()))?;
        if !response.status().is_success() {
            return Err(PortError::Network(format!(
                "sing-box asset download returned HTTP {}",
                response.status()
            )));
        }
        let total = response.content_length();
        if total.is_some_and(|length| length > MAX_ARCHIVE_BYTES) {
            return Err(invalid_installation(
                "sing-box release archive exceeds the configured size bound",
            ));
        }
        let mut downloaded = 0u64;
        let mut bytes = Vec::with_capacity(total.unwrap_or_default().min(8 * 1024 * 1024) as usize);
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            if progress.is_cancelled() {
                return Err(cancelled_install());
            }
            let chunk = chunk.map_err(|error| PortError::Network(error.to_string()))?;
            downloaded = downloaded.saturating_add(chunk.len() as u64);
            if downloaded > MAX_ARCHIVE_BYTES {
                return Err(invalid_installation(
                    "sing-box release archive exceeded the configured size bound",
                ));
            }
            bytes.extend_from_slice(&chunk);
            progress.progress(VersionDownloadProgress { downloaded, total });
        }
        Ok(bytes)
    }
}

#[async_trait]
pub(super) trait SingBoxBinaryInspector: Send + Sync {
    async fn version(&self, binary: &Path) -> Result<CoreVersion, PortError>;
}

pub(super) struct ProcessSingBoxBinaryInspector;

#[async_trait]
impl SingBoxBinaryInspector for ProcessSingBoxBinaryInspector {
    async fn version(&self, binary: &Path) -> Result<CoreVersion, PortError> {
        let output = Command::new(binary)
            .arg("-v")
            .output()
            .await
            .map_err(|error| PortError::Failed(format!("run sing-box version probe: {error}")))?;
        if !output.status.success() {
            return Err(PortError::Failed(format!(
                "sing-box version probe exited with {}",
                output.status
            )));
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        parse_binary_version(&stdout)
    }
}

pub(super) fn parse_binary_version(output: &str) -> Result<CoreVersion, PortError> {
    let first_line = output.lines().next().unwrap_or_default().trim();
    let version = first_line
        .strip_prefix("sing-box version ")
        .or_else(|| first_line.strip_prefix("sing-box "))
        .ok_or_else(|| {
            PortError::Failed("sing-box version output has an unexpected engine identity".into())
        })?;
    let version = version.split_whitespace().next().unwrap_or_default();
    let canonical = if version.starts_with('v') {
        version.to_owned()
    } else {
        format!("v{version}")
    };
    validate_version(&canonical)?;
    CoreVersion::new(canonical).map_err(|error| PortError::Failed(error.to_string()))
}

pub(super) struct InstallPlan {
    pub(super) asset: LockedSingBoxAsset,
}

pub(super) struct DownloadedArchive {
    asset: LockedSingBoxAsset,
    bytes: Vec<u8>,
}

pub(super) struct VerifiedArchive {
    asset: LockedSingBoxAsset,
    bytes: Vec<u8>,
}

pub(super) struct ExtractedBinary {
    asset: LockedSingBoxAsset,
    bytes: Vec<u8>,
}

pub(super) struct VerifiedBinary {
    asset: LockedSingBoxAsset,
    bytes: Vec<u8>,
    receipt: InstalledSingBoxReceipt,
}

pub(super) struct StagedInstall {
    temp_dir: PathBuf,
    destination: PathBuf,
    receipt: InstalledSingBoxReceipt,
}

pub(super) enum InstallStage {
    Planned(InstallPlan),
    Downloaded(DownloadedArchive),
    ArchiveVerified(VerifiedArchive),
    BinaryExtracted(ExtractedBinary),
    BinaryVerified(VerifiedBinary),
    Staged(StagedInstall),
    Published { version: String },
}

pub(super) enum PublishedInstall {
    Created { version: String },
    AlreadyPresent { version: String },
}

impl InstallStage {
    pub(super) async fn download(
        self,
        downloader: &dyn SingBoxArchiveDownloader,
        progress: &dyn VersionProgressSink,
    ) -> Result<Self, PortError> {
        match self {
            Self::Planned(plan) => {
                let bytes = downloader.download(&plan.asset, progress).await?;
                if progress.is_cancelled() {
                    return Err(cancelled_install());
                }
                Ok(Self::Downloaded(DownloadedArchive {
                    asset: plan.asset,
                    bytes,
                }))
            }
            Self::Downloaded(_)
            | Self::ArchiveVerified(_)
            | Self::BinaryExtracted(_)
            | Self::BinaryVerified(_)
            | Self::Staged(_)
            | Self::Published { .. } => Err(invalid_installation(
                "sing-box install transaction attempted an invalid download transition",
            )),
        }
    }

    pub(super) fn verify_archive(self) -> Result<Self, PortError> {
        match self {
            Self::Downloaded(downloaded) => {
                let digest = sha256_hex(&downloaded.bytes);
                if digest != downloaded.asset.archive_sha256 {
                    return Err(invalid_installation(format!(
                        "sing-box archive digest mismatch: expected {}, observed {digest}",
                        downloaded.asset.archive_sha256
                    )));
                }
                Ok(Self::ArchiveVerified(VerifiedArchive {
                    asset: downloaded.asset,
                    bytes: downloaded.bytes,
                }))
            }
            Self::Planned(_)
            | Self::ArchiveVerified(_)
            | Self::BinaryExtracted(_)
            | Self::BinaryVerified(_)
            | Self::Staged(_)
            | Self::Published { .. } => Err(invalid_installation(
                "sing-box install transaction attempted an invalid archive verification transition",
            )),
        }
    }

    pub(super) fn extract_binary(self) -> Result<Self, PortError> {
        match self {
            Self::ArchiveVerified(verified) => {
                let bytes = extract_sing_box_binary(&verified.bytes)?;
                Ok(Self::BinaryExtracted(ExtractedBinary {
                    asset: verified.asset,
                    bytes,
                }))
            }
            Self::Planned(_)
            | Self::Downloaded(_)
            | Self::BinaryExtracted(_)
            | Self::BinaryVerified(_)
            | Self::Staged(_)
            | Self::Published { .. } => Err(invalid_installation(
                "sing-box install transaction attempted an invalid extraction transition",
            )),
        }
    }

    pub(super) async fn verify_binary(
        self,
        inspector: &dyn SingBoxBinaryInspector,
        target: SingBoxBinaryTarget,
        probe_directory: &Path,
    ) -> Result<Self, PortError> {
        match self {
            Self::BinaryExtracted(extracted) => {
                if !extracted.bytes.starts_with(b"\x7fELF") {
                    return Err(invalid_installation(
                        "locked sing-box archive did not contain an ELF executable",
                    ));
                }
                fs::create_dir_all(probe_directory)
                    .await
                    .map_err(|error| PortError::Io(error.to_string()))?;
                let probe_path = unique_path(probe_directory, ".sing-box-version-probe", "bin");
                let probe_result = async {
                    fs::write(&probe_path, &extracted.bytes)
                        .await
                        .map_err(|error| PortError::Io(error.to_string()))?;
                    set_executable(&probe_path).await?;
                    inspector.version(&probe_path).await
                }
                .await;
                let cleanup_result = fs::remove_file(&probe_path).await;
                let reported = match (probe_result, cleanup_result) {
                    (Ok(version), Ok(())) => version,
                    (Err(error), Ok(())) => return Err(error),
                    (Ok(_), Err(error)) => return Err(PortError::Io(error.to_string())),
                    (Err(error), Err(cleanup)) => {
                        return Err(PortError::Io(format!(
                            "{error}; sing-box version probe cleanup failed: {cleanup}"
                        )));
                    }
                };
                if reported.raw() != extracted.asset.version {
                    return Err(invalid_installation(format!(
                        "locked sing-box version {} differs from binary-reported {}",
                        extracted.asset.version,
                        reported.raw()
                    )));
                }
                let receipt = InstalledSingBoxReceipt {
                    schema_version: 1,
                    engine_id: engine_id().as_str().to_owned(),
                    version: extracted.asset.version.clone(),
                    target: target.as_triple().to_owned(),
                    adapter_revision: extracted.asset.adapter_revision,
                    archive: extracted.asset.archive.clone(),
                    archive_sha256: extracted.asset.archive_sha256.clone(),
                    binary_sha256: sha256_hex(&extracted.bytes),
                };
                Ok(Self::BinaryVerified(VerifiedBinary {
                    asset: extracted.asset,
                    bytes: extracted.bytes,
                    receipt,
                }))
            }
            Self::Planned(_)
            | Self::Downloaded(_)
            | Self::ArchiveVerified(_)
            | Self::BinaryVerified(_)
            | Self::Staged(_)
            | Self::Published { .. } => Err(invalid_installation(
                "sing-box install transaction attempted an invalid binary validation transition",
            )),
        }
    }

    pub(super) async fn stage(self, backend: &SingBoxVersionBackend) -> Result<Self, PortError> {
        match self {
            Self::BinaryVerified(verified) => {
                let versions = backend.versions_dir();
                fs::create_dir_all(&versions)
                    .await
                    .map_err(|error| PortError::Io(error.to_string()))?;
                let temp_dir = unique_path(&versions, ".installing", "tmp");
                fs::create_dir(&temp_dir)
                    .await
                    .map_err(|error| PortError::Io(error.to_string()))?;
                let destination = backend.version_dir(&verified.asset.version)?;
                let staged = StagedInstall {
                    temp_dir,
                    destination,
                    receipt: verified.receipt,
                };
                let write_result = async {
                    let binary_path = staged.temp_dir.join("sing-box");
                    fs::write(&binary_path, &verified.bytes)
                        .await
                        .map_err(|error| PortError::Io(error.to_string()))?;
                    set_executable(&binary_path).await?;
                    write_json_atomically(&staged.temp_dir.join("install.json"), &staged.receipt)
                        .await
                }
                .await;
                if let Err(error) = write_result {
                    return Err(staged.cleanup(error).await);
                }
                Ok(Self::Staged(staged))
            }
            Self::Planned(_)
            | Self::Downloaded(_)
            | Self::ArchiveVerified(_)
            | Self::BinaryExtracted(_)
            | Self::Staged(_)
            | Self::Published { .. } => Err(invalid_installation(
                "sing-box install transaction attempted an invalid staging transition",
            )),
        }
    }

    pub(super) async fn publish(
        self,
        backend: &SingBoxVersionBackend,
    ) -> Result<PublishedInstall, PortError> {
        match self {
            Self::Staged(staged) => {
                let destination_exists = match fs::try_exists(&staged.destination).await {
                    Ok(exists) => exists,
                    Err(error) => {
                        return Err(staged.cleanup(PortError::Io(error.to_string())).await);
                    }
                };
                if destination_exists {
                    let existing = backend.verify_installed(&staged.receipt.version).await;
                    let cleanup = fs::remove_dir_all(&staged.temp_dir).await;
                    match (existing, cleanup) {
                        (Ok(installed), Ok(())) if installed.receipt == staged.receipt => {
                            Ok(PublishedInstall::AlreadyPresent {
                                version: staged.receipt.version,
                            })
                        }
                        (Err(error), Ok(())) => Err(error),
                        (Err(error), Err(cleanup)) => Err(PortError::Io(format!(
                            "{error}; duplicate sing-box staging cleanup failed: {cleanup}"
                        ))),
                        (Ok(_), Err(error)) => Err(PortError::Io(format!(
                            "sing-box installation already exists but staging cleanup failed: {error}"
                        ))),
                        (Ok(_), Ok(())) => Err(invalid_installation(
                            "existing sing-box installation has a different target-bound receipt",
                        )),
                    }
                } else {
                    match fs::rename(&staged.temp_dir, &staged.destination).await {
                        Ok(()) => Ok(PublishedInstall::Created {
                            version: staged.receipt.version,
                        }),
                        Err(error) => Err(staged.cleanup(PortError::Io(error.to_string())).await),
                    }
                }
            }
            Self::Planned(_)
            | Self::Downloaded(_)
            | Self::ArchiveVerified(_)
            | Self::BinaryExtracted(_)
            | Self::BinaryVerified(_)
            | Self::Published { .. } => Err(invalid_installation(
                "sing-box install transaction attempted an invalid publication transition",
            )),
        }
    }

    pub(super) async fn cancel(self) -> Result<(), PortError> {
        match self {
            Self::Staged(staged) => Err(staged.cleanup(cancelled_install()).await),
            Self::Planned(_)
            | Self::Downloaded(_)
            | Self::ArchiveVerified(_)
            | Self::BinaryExtracted(_)
            | Self::BinaryVerified(_)
            | Self::Published { .. } => Err(cancelled_install()),
        }
    }
}

impl StagedInstall {
    async fn cleanup(self, error: PortError) -> PortError {
        match fs::remove_dir_all(&self.temp_dir).await {
            Ok(()) => error,
            Err(cleanup) if cleanup.kind() == ErrorKind::NotFound => error,
            Err(cleanup) => PortError::Io(format!(
                "{error}; temporary sing-box installation cleanup failed at {}: {cleanup}",
                self.temp_dir.display()
            )),
        }
    }
}

async fn set_executable(path: &Path) -> Result<(), PortError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path)
            .await
            .map_err(|error| PortError::Io(error.to_string()))?
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions)
            .await
            .map_err(|error| PortError::Io(error.to_string()))
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Err(unsupported_target(
            "sing-box locked archives are not executable on this host",
        ))
    }
}

fn extract_sing_box_binary(archive_bytes: &[u8]) -> Result<Vec<u8>, PortError> {
    let decoder = GzDecoder::new(archive_bytes);
    let mut archive = Archive::new(decoder);
    let entries = archive
        .entries()
        .map_err(|error| invalid_installation(format!("invalid sing-box tar archive: {error}")))?;
    let mut binary = None;
    for entry in entries {
        let mut entry = entry.map_err(|error| {
            invalid_installation(format!("invalid sing-box tar entry: {error}"))
        })?;
        let path = entry.path().map_err(|error| {
            invalid_installation(format!("invalid sing-box archive path: {error}"))
        })?;
        if path.file_name().and_then(|name| name.to_str()) != Some("sing-box") {
            continue;
        }
        if !entry.header().entry_type().is_file() || binary.is_some() {
            return Err(invalid_installation(
                "sing-box archive must contain exactly one regular sing-box executable",
            ));
        }
        let mut contents = Vec::new();
        entry.read_to_end(&mut contents).map_err(|error| {
            invalid_installation(format!("cannot read sing-box executable: {error}"))
        })?;
        binary = Some(contents);
    }
    binary.ok_or_else(|| {
        PortError::NotFound("locked sing-box archive has no regular sing-box executable".to_owned())
    })
}
