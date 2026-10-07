//! Durable, non-overwriting desktop log exports under the host's config directory.
use async_trait::async_trait;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::log_export::{LogExportArtifact, LogExportReceipt, MAX_LOG_EXPORT_BYTES};
use infiltrator_domain::profile_source::hash_document_bytes;
use infiltrator_ports::error::PortError;
use infiltrator_ports::log_export::LogExportPort;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use tempfile::Builder;
use tokio::task::spawn_blocking;

#[derive(Clone)]
pub struct DesktopLogExportPort {
    directory: PathBuf,
}
impl DesktopLogExportPort {
    pub fn new(config_dir: impl AsRef<Path>) -> Self {
        Self {
            directory: config_dir.as_ref().join("exports"),
        }
    }
    fn publish(&self, artifact: LogExportArtifact) -> Result<LogExportReceipt, PortError> {
        let summary = &artifact.summary;
        summary.validate().map_err(PortError::Rejected)?;
        let identity = &summary.identity;
        if artifact.content.len() != summary.bytes
            || summary.bytes > MAX_LOG_EXPORT_BYTES
            || hash_document_bytes(&artifact.content) != identity.sha256
            || identity.sequence == 0
            || !identity.session.token.is_valid()
        {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::InvalidInput,
                "Log export bytes do not match their prepared identity",
                false,
            )));
        }
        fs::create_dir_all(&self.directory).map_err(storage)?;
        let target = self.directory.join(format!(
            "logs-{:x}-{:032x}-{:x}-{}.log",
            identity.session.generation,
            identity.session.token.value(),
            identity.sequence,
            identity.sha256
        ));
        if target.exists() {
            return existing(&target, &artifact);
        }
        let mut file = Builder::new()
            .prefix(".musicfrog-logs-")
            .tempfile_in(&self.directory)
            .map_err(storage)?;
        file.write_all(artifact.content.as_bytes())
            .map_err(storage)?;
        file.as_file().sync_all().map_err(storage)?;
        match file.persist_noclobber(&target) {
            Ok(published) => Ok(LogExportReceipt {
                summary: artifact.summary,
                path: target.to_string_lossy().into_owned(),
                bytes_written: published.metadata().map_err(storage)?.len() as usize,
            }),
            Err(error) if error.error.kind() == io::ErrorKind::AlreadyExists => {
                existing(&target, &artifact)
            }
            Err(error) => Err(storage(error.error)),
        }
    }
}
fn existing(target: &Path, artifact: &LogExportArtifact) -> Result<LogExportReceipt, PortError> {
    let metadata = fs::symlink_metadata(target).map_err(storage)?;
    if !metadata.is_file() || metadata.len() != artifact.summary.bytes as u64 {
        return Err(collision());
    }
    let content = fs::read(target).map_err(storage)?;
    if content != artifact.content.as_bytes() {
        return Err(collision());
    }
    Ok(LogExportReceipt {
        summary: artifact.summary.clone(),
        path: target.to_string_lossy().into_owned(),
        bytes_written: content.len(),
    })
}
fn collision() -> PortError {
    PortError::Rejected(Failure::new(
        ErrorCode::Storage,
        "Log export destination already contains different data",
        false,
    ))
}
fn storage(error: io::Error) -> PortError {
    if error.kind() == io::ErrorKind::PermissionDenied {
        PortError::PermissionDenied(error.to_string())
    } else {
        PortError::Io(error.to_string())
    }
}
#[async_trait]
impl LogExportPort for DesktopLogExportPort {
    async fn save(&self, artifact: LogExportArtifact) -> Result<LogExportReceipt, PortError> {
        let writer = self.clone();
        spawn_blocking(move || writer.publish(artifact))
            .await
            .map_err(|_| PortError::Failed("Log export worker did not complete".into()))?
    }
}

#[cfg(test)]
#[path = "log_export_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "log_export_core_tests.rs"]
mod core_tests;
