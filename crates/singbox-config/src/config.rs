//! Native sing-box JSON validation and exact config-file readback.

use crate::profile_document_codec::SingBoxProfileDocumentCodec;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::profile_document::{DocumentDiagnosticSnapshot, DocumentValidation};
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_document_codec::ProfileDocumentCodecPort;
use singbox_process::command::{NativeConfigCheck, SingBoxCommandPort, TokioSingBoxCommandPort};
#[cfg(test)]
use std::ffi::OsStr;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};
use std::process;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::fs::{self, OpenOptions};
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;

static TEMP_CONFIG_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SingBoxConfigApplyReceipt {
    pub persisted_byte_count: usize,
}

/// Exact pre-transaction state of the active native JSON file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SingBoxConfigFileState {
    Missing,
    Present(Vec<u8>),
}

/// Owns sing-box native validation and its active JSON file.
///
/// `apply_and_readback` validates before replacing the file, installs the
/// exact submitted bytes atomically, and verifies those bytes from disk. The
/// caller still owns process restart/readiness and must not treat this file
/// receipt as proof that a running core loaded the document. Transactions are
/// serialized per adapter instance. A config path must have exactly one
/// adapter owner; separate instances or processes sharing it are not locked.
pub struct SingBoxProfileConfigurationAdapter {
    binary_path: PathBuf,
    config_path: PathBuf,
    command: Arc<dyn SingBoxCommandPort>,
    codec: SingBoxProfileDocumentCodec,
    apply_lock: Mutex<()>,
}

impl SingBoxProfileConfigurationAdapter {
    pub fn with_tokio(binary_path: PathBuf, config_path: PathBuf) -> Self {
        Self::new(binary_path, config_path, Arc::new(TokioSingBoxCommandPort))
    }

    pub fn new(
        binary_path: PathBuf,
        config_path: PathBuf,
        command: Arc<dyn SingBoxCommandPort>,
    ) -> Self {
        Self {
            binary_path,
            config_path,
            command,
            codec: SingBoxProfileDocumentCodec,
            apply_lock: Mutex::new(()),
        }
    }

    pub async fn validate_native(&self, raw_bytes: &[u8]) -> Result<DocumentValidation, PortError> {
        match self.codec.inspect(raw_bytes)? {
            DocumentValidation::Invalid { diagnostics } => {
                Ok(DocumentValidation::Invalid { diagnostics })
            }
            DocumentValidation::Valid => {
                let staged = create_temporary_config(&self.config_path, raw_bytes).await?;
                let check = self
                    .command
                    .check_config(&self.binary_path, staged.path())
                    .await;
                drop(staged);
                match check? {
                    NativeConfigCheck::Accepted => Ok(DocumentValidation::Valid),
                    NativeConfigCheck::Rejected => Ok(DocumentValidation::Invalid {
                        diagnostics: vec![
                            DocumentDiagnosticSnapshot {
                                line: None,
                                column: None,
                                message: "sing-box native config check rejected this document"
                                    .to_owned(),
                            },
                        ],
                    }),
                }
            }
        }
    }

    pub async fn apply_and_readback(
        &self,
        raw_bytes: &[u8],
    ) -> Result<SingBoxConfigApplyReceipt, PortError> {
        let _transaction = self.apply_lock.lock().await;
        let validated = SubmittedConfig(raw_bytes.to_vec())
            .after_native_validation(self)
            .await?;
        let captured = validated.capture_previous(&self.config_path).await?;
        let installed = captured.install(&self.config_path).await?;
        match installed.verify_readback(&self.config_path).await {
            Ok(receipt) => Ok(receipt),
            Err(failure) => {
                let rollback = restore_previous_config(&self.config_path, failure.previous).await;
                Err(readback_failure(failure.cause, rollback))
            }
        }
    }

    pub async fn readback(&self) -> Result<Vec<u8>, PortError> {
        fs::read(&self.config_path)
            .await
            .map_err(|error| map_storage_error(&self.config_path, error))
    }

    pub async fn file_state(&self) -> Result<SingBoxConfigFileState, PortError> {
        let _transaction = self.apply_lock.lock().await;
        read_previous_config(&self.config_path).await
    }

    /// Restores a transaction snapshot and verifies the resulting file state.
    pub async fn restore_file_state(
        &self,
        expected: &SingBoxConfigFileState,
    ) -> Result<(), PortError> {
        let _transaction = self.apply_lock.lock().await;
        restore_previous_config(&self.config_path, expected.clone()).await?;
        let observed = read_previous_config(&self.config_path).await?;
        match classify_exact_readback(observed, expected) {
            ExactReadback::Exact(_) => Ok(()),
            ExactReadback::Different => Err(PortError::Failed(
                "sing-box native config rollback readback differed from the previous file state"
                    .to_owned(),
            )),
        }
    }
}

struct SubmittedConfig(Vec<u8>);

struct ValidatedConfig(Vec<u8>);

struct PreviousConfigCaptured {
    raw_bytes: Vec<u8>,
    previous: SingBoxConfigFileState,
}

struct InstalledConfig {
    raw_bytes: Vec<u8>,
    previous: SingBoxConfigFileState,
}

struct FailedConfigReadback {
    previous: SingBoxConfigFileState,
    cause: ConfigReadbackFailure,
}

enum ConfigReadbackFailure {
    BytesDiffer,
    Unavailable(PortError),
}

enum ExactReadback<T> {
    Exact(T),
    Different,
}

impl SubmittedConfig {
    async fn after_native_validation(
        self,
        adapter: &SingBoxProfileConfigurationAdapter,
    ) -> Result<ValidatedConfig, PortError> {
        match adapter.validate_native(&self.0).await? {
            DocumentValidation::Valid => Ok(ValidatedConfig(self.0)),
            DocumentValidation::Invalid { .. } => Err(PortError::Rejected(Failure::new(
                ErrorCode::Configuration,
                "sing-box rejected the native JSON configuration",
                false,
            ))),
        }
    }
}

impl ValidatedConfig {
    async fn capture_previous(
        self,
        config_path: &Path,
    ) -> Result<PreviousConfigCaptured, PortError> {
        Ok(PreviousConfigCaptured {
            raw_bytes: self.0,
            previous: read_previous_config(config_path).await?,
        })
    }
}

impl PreviousConfigCaptured {
    async fn install(self, config_path: &Path) -> Result<InstalledConfig, PortError> {
        atomic_write(config_path, &self.raw_bytes).await?;
        Ok(InstalledConfig {
            raw_bytes: self.raw_bytes,
            previous: self.previous,
        })
    }
}

impl InstalledConfig {
    async fn verify_readback(
        self,
        config_path: &Path,
    ) -> Result<SingBoxConfigApplyReceipt, FailedConfigReadback> {
        match fs::read(config_path).await {
            Ok(readback) => match classify_exact_readback(readback, &self.raw_bytes) {
                ExactReadback::Exact(readback) => Ok(SingBoxConfigApplyReceipt {
                    persisted_byte_count: readback.len(),
                }),
                ExactReadback::Different => Err(FailedConfigReadback {
                    previous: self.previous,
                    cause: ConfigReadbackFailure::BytesDiffer,
                }),
            },
            Err(error) => Err(FailedConfigReadback {
                previous: self.previous,
                cause: ConfigReadbackFailure::Unavailable(map_storage_error(config_path, error)),
            }),
        }
    }
}

fn classify_exact_readback<T: PartialEq>(observed: T, expected: &T) -> ExactReadback<T> {
    if &observed == expected {
        ExactReadback::Exact(observed)
    } else {
        ExactReadback::Different
    }
}

struct TemporaryConfigFile {
    path: PathBuf,
}

impl TemporaryConfigFile {
    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TemporaryConfigFile {
    fn drop(&mut self) {
        use std::fs;
        let _ = fs::remove_file(&self.path);
    }
}

async fn create_temporary_config(
    active_path: &Path,
    raw_bytes: &[u8],
) -> Result<TemporaryConfigFile, PortError> {
    let parent = active_path
        .parent()
        .ok_or_else(|| PortError::Io("sing-box config path has no parent directory".to_owned()))?;
    ensure_directory(parent).await?;
    let file_name = active_path
        .file_name()
        .map(|name| name.to_string_lossy())
        .unwrap_or_else(|| "config.json".into());

    for _ in 0..32 {
        let sequence = TEMP_CONFIG_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let candidate = parent.join(format!(
            ".{file_name}.candidate-{}-{sequence}.json",
            process::id()
        ));
        let open = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
            .await;
        match open {
            Ok(mut file) => {
                let write_result = async {
                    file.write_all(raw_bytes).await?;
                    file.sync_all().await
                }
                .await;
                if let Err(error) = write_result {
                    let _ = fs::remove_file(&candidate).await;
                    return Err(map_storage_error(&candidate, error));
                }
                return Ok(TemporaryConfigFile { path: candidate });
            }
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
            Err(error) => return Err(map_storage_error(&candidate, error)),
        }
    }

    Err(PortError::Io(
        "could not reserve a temporary sing-box config file".to_owned(),
    ))
}

async fn atomic_write(active_path: &Path, raw_bytes: &[u8]) -> Result<(), PortError> {
    let staged = create_temporary_config(active_path, raw_bytes).await?;
    fs::rename(staged.path(), active_path)
        .await
        .map_err(|error| map_storage_error(active_path, error))?;
    Ok(())
}

async fn read_previous_config(path: &Path) -> Result<SingBoxConfigFileState, PortError> {
    match fs::read(path).await {
        Ok(bytes) => Ok(SingBoxConfigFileState::Present(bytes)),
        Err(error) if error.kind() == ErrorKind::NotFound => {
            Ok(SingBoxConfigFileState::Missing)
        }
        Err(error) => Err(map_storage_error(path, error)),
    }
}

async fn restore_previous_config(
    active_path: &Path,
    previous: SingBoxConfigFileState,
) -> Result<(), PortError> {
    match previous {
        SingBoxConfigFileState::Missing => match fs::remove_file(active_path).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
            Err(error) => Err(map_storage_error(active_path, error)),
        },
        SingBoxConfigFileState::Present(bytes) => atomic_write(active_path, &bytes).await,
    }
}

async fn ensure_directory(path: &Path) -> Result<(), PortError> {
    match fs::metadata(path).await {
        Ok(metadata) if metadata.is_dir() => Ok(()),
        Ok(_) => Err(PortError::Io(
            "sing-box config parent is not a directory".to_owned(),
        )),
        Err(error) => Err(map_storage_error(path, error)),
    }
}

fn readback_failure(cause: ConfigReadbackFailure, rollback: Result<(), PortError>) -> PortError {
    match (cause, rollback) {
        (ConfigReadbackFailure::BytesDiffer, Ok(())) => PortError::Io(
            "sing-box config readback differed from the submitted bytes; previous file restored"
                .to_owned(),
        ),
        (ConfigReadbackFailure::Unavailable(cause), Ok(())) => PortError::Io(format!(
            "sing-box config readback failed; previous file restored: {cause}"
        )),
        (cause, Err(rollback)) => PortError::Failed(format!(
            "sing-box config readback failed and previous file restoration failed: {} / {rollback}",
            readback_cause(&cause)
        )),
    }
}

fn readback_cause(cause: &ConfigReadbackFailure) -> &'static str {
    match cause {
        ConfigReadbackFailure::BytesDiffer => "readback bytes differed",
        ConfigReadbackFailure::Unavailable(_) => "readback was unavailable",
    }
}

fn map_storage_error(path: &Path, error: io::Error) -> PortError {
    match error.kind() {
        ErrorKind::NotFound => PortError::NotFound(format!(
            "sing-box config file is unavailable: {}",
            path.display()
        )),
        ErrorKind::PermissionDenied => {
            PortError::PermissionDenied("sing-box config file cannot be accessed".to_owned())
        }
        _ => PortError::Io(format!("sing-box config file operation failed: {error}")),
    }
}

#[cfg(test)]
mod transaction_recovery_tests {
    use super::*;
    use tempfile::TempDir;

    #[derive(Clone, Copy)]
    enum ReadbackFault {
        DifferentBytes,
        MissingFile,
    }

    #[derive(Clone, Copy)]
    enum PreviousFile {
        Missing,
        Present(&'static [u8]),
    }

    impl PreviousFile {
        fn into_state(self) -> SingBoxConfigFileState {
            match self {
                Self::Missing => SingBoxConfigFileState::Missing,
                Self::Present(bytes) => SingBoxConfigFileState::Present(bytes.to_vec()),
            }
        }

        fn assert_restored(self, path: &Path) {
            match self {
                Self::Missing => assert!(
                    !path.exists(),
                    "failed transaction must remove a newly-created config"
                ),
                Self::Present(expected) => {
                    use std::fs;
                    assert_eq!(fs::read(path).unwrap(), expected);
                }
            }
        }
    }

    #[tokio::test]
    async fn failed_config_readback_restores_previous_bytes_or_removes_new_file() {
        for fault in [ReadbackFault::DifferentBytes, ReadbackFault::MissingFile] {
            for previous in [
                PreviousFile::Missing,
                PreviousFile::Present(b"previous-config"),
            ] {
                let home = TempDir::new().unwrap();
                let config_path = home.path().join("config.json");
                let proposed = b"submitted-config";
                fs::write(&config_path, proposed).await.unwrap();
                match fault {
                    ReadbackFault::DifferentBytes => {
                        fs::write(&config_path, b"externally-changed")
                            .await
                            .unwrap();
                    }
                    ReadbackFault::MissingFile => {
                        fs::remove_file(&config_path).await.unwrap();
                    }
                }

                let installed = InstalledConfig {
                    raw_bytes: proposed.to_vec(),
                    previous: previous.into_state(),
                };
                let failure = installed
                    .verify_readback(&config_path)
                    .await
                    .expect_err("the injected readback fault must fail verification");
                let reported = readback_failure(
                    failure.cause,
                    restore_previous_config(&config_path, failure.previous).await,
                );

                assert!(matches!(reported, PortError::Io(_)));
                previous.assert_restored(&config_path);
                let mut entries = fs::read_dir(home.path()).await.unwrap();
                let first = entries.next_entry().await.unwrap();
                match previous {
                    PreviousFile::Missing => assert!(first.is_none()),
                    PreviousFile::Present(_) => {
                        assert_eq!(
                            first.unwrap().file_name(),
                            OsStr::new("config.json")
                        );
                        assert!(entries.next_entry().await.unwrap().is_none());
                    }
                }
            }
        }
    }
}
