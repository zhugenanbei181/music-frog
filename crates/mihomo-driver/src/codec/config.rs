//! Native Mihomo `-t` configuration dry-run behind the command boundary.
//!
//! The candidate is staged beside the active configuration so relative
//! resource paths resolve exactly as they will after publication, then the
//! exact selected binary performs its own `-t` config check. This is the
//! engine's syntax/semantics authority for a stopped-state candidate; a
//! passing check does not mean a running core loaded the document.

use crate::process::command::{MihomoCommandPort, NativeConfigCheck, TokioMihomoCommandPort};
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_ports::error::PortError;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::id;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::fs::{File, OpenOptions, remove_file};
use tokio::io::AsyncWriteExt;

static CANDIDATE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Owns the native dry-run for one selected binary and configuration directory.
pub struct MihomoNativeConfigCheck {
    binary_path: PathBuf,
    config_dir: PathBuf,
    command: Arc<dyn MihomoCommandPort>,
}

impl MihomoNativeConfigCheck {
    /// Creates the dry-run owner that launches the binary through tokio.
    pub fn with_tokio(binary_path: PathBuf, config_dir: PathBuf) -> Self {
        Self::new(binary_path, config_dir, Arc::new(TokioMihomoCommandPort))
    }

    /// Creates the dry-run owner with an injected command boundary.
    pub fn new(
        binary_path: PathBuf,
        config_dir: PathBuf,
        command: Arc<dyn MihomoCommandPort>,
    ) -> Self {
        Self {
            binary_path,
            config_dir,
            command,
        }
    }

    /// Stages `document_bytes` beside the active profile and runs the exact
    /// selected binary's `-t` check against it.
    pub async fn validate(&self, document_bytes: &[u8]) -> Result<(), PortError> {
        let (candidate, file) = reserve_candidate(&self.config_dir).await?;
        let path = candidate.0.clone();
        let validation = self.write_and_check(file, &path, document_bytes).await;
        let cleanup = remove_file(&path).await;
        match (validation, cleanup) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(error), Ok(())) => Err(error),
            (Ok(()), Err(error)) => Err(PortError::Io(format!(
                "failed to remove Mihomo native-validation candidate: {error}"
            ))),
            (Err(validation), Err(cleanup)) => Err(PortError::Rejected(Failure::new(
                ErrorCode::Configuration,
                format!(
                    "{}; failed to remove Mihomo native-validation candidate: {cleanup}",
                    validation
                ),
                false,
            ))),
        }
    }

    async fn write_and_check(
        &self,
        mut file: File,
        path: &Path,
        document_bytes: &[u8],
    ) -> Result<(), PortError> {
        file.write_all(document_bytes)
            .await
            .map_err(|error| PortError::Io(error.to_string()))?;
        file.flush()
            .await
            .map_err(|error| PortError::Io(error.to_string()))?;
        drop(file);
        let outcome = self
            .command
            .check_config(&self.binary_path, path, &self.config_dir)
            .await?;
        match outcome.check {
            NativeConfigCheck::Accepted => Ok(()),
            NativeConfigCheck::Rejected => Err(rejected(outcome.detail)),
        }
    }
}

fn rejected(detail: String) -> PortError {
    PortError::Rejected(Failure::new(
        ErrorCode::Configuration,
        if detail.is_empty() {
            "selected Mihomo binary rejected candidate config".to_owned()
        } else {
            format!("selected Mihomo binary rejected candidate config: {detail}")
        },
        false,
    ))
}

struct NativeValidationCandidate(PathBuf);

impl Drop for NativeValidationCandidate {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

async fn reserve_candidate(
    config_dir: &Path,
) -> Result<(NativeValidationCandidate, File), PortError> {
    for _ in 0..16 {
        let sequence = CANDIDATE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = config_dir.join(format!(
            ".mihomo-profile-validation-{}-{sequence}.yaml",
            id()
        ));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .await
        {
            Ok(file) => return Ok((NativeValidationCandidate(path), file)),
            Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(PortError::Io(error.to_string())),
        }
    }
    Err(PortError::Failed(
        "could not reserve a unique Mihomo native-validation candidate".to_owned(),
    ))
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
