//! Durable whole-document and sidecar operations shared by engine stores.

use infiltrator_platform::durable_replace::{DurableReplaceError, remove_durable, replace_durable};
use infiltrator_ports::error::PortError;
use std::fs::OpenOptions as StdOpenOptions;
use std::io::{Error, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::process::id;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::fs::{OpenOptions, create_dir_all, read, remove_file};
use tokio::io::AsyncWriteExt;

static NEXT_TEMPORARY: AtomicU64 = AtomicU64::new(0);
const DURABILITY_UNCERTAIN_PREFIX: &str =
    "document replacement succeeded but directory durability is uncertain";
const REMOVAL_DURABILITY_UNCERTAIN_PREFIX: &str =
    "document removal succeeded but directory durability is uncertain";
pub const RECORD_DURABILITY_UNCERTAIN_PREFIX: &str =
    "record rename succeeded but directory durability is uncertain";

struct TemporaryDocumentCleanup(PathBuf);

impl Drop for TemporaryDocumentCleanup {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Missing files alone are represented as `None`; permission and I/O errors stay explicit.
pub async fn read_optional(path: &Path) -> Result<Option<String>, PortError> {
    let Some(bytes) = read_optional_bytes(path).await? else {
        return Ok(None);
    };
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|error| PortError::Io(error.to_string()))
}

/// Read the exact persisted bytes without interpreting their engine format.
pub async fn read_optional_bytes(path: &Path) -> Result<Option<Vec<u8>>, PortError> {
    match read(path).await {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(io_error(error)),
    }
}

/// Remove an optional document. A missing file is already the requested state.
pub async fn remove_optional(path: &Path) -> Result<(), PortError> {
    match remove_file(path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io_error(error)),
    }
}

/// Remove an optional document and durably publish the directory change.
pub fn remove_optional_durable(path: &Path) -> Result<(), PortError> {
    match remove_durable(path) {
        Ok(()) => Ok(()),
        Err(DurableReplaceError::NotCommitted(error)) => Err(io_error(error)),
        Err(DurableReplaceError::DurabilityUncertain(error)) => Err(PortError::Failed(format!(
            "{REMOVAL_DURABILITY_UNCERTAIN_PREFIX}: {error}"
        ))),
    }
}

/// Replace a UTF-8 document through the same durable byte path used by workspaces.
pub async fn write_document(path: &Path, text: &str) -> Result<(), PortError> {
    write_document_bytes(path, text.as_bytes()).await
}

/// Replace opaque document bytes through a fully flushed same-directory temporary.
pub async fn write_document_bytes(path: &Path, content: &[u8]) -> Result<(), PortError> {
    write_document_bytes_durable(path, content).await
}

/// Flush a staged file and use the platform's durable atomic replacement.
pub async fn write_document_bytes_durable(path: &Path, content: &[u8]) -> Result<(), PortError> {
    let parent = path
        .parent()
        .ok_or_else(|| PortError::Io("document path has no parent".into()))?;
    create_dir_all(parent).await.map_err(io_error)?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| PortError::Io("document path has no filename".into()))?;
    let (temporary, mut file) = loop {
        let sequence = NEXT_TEMPORARY.fetch_add(1, Ordering::Relaxed);
        let temporary = parent.join(format!(".{name}.tmp-{}-{sequence}", id()));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .await
        {
            Ok(file) => break (temporary, file),
            Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(io_error(error)),
        }
    };
    let _temporary_cleanup = TemporaryDocumentCleanup(temporary.clone());
    let staged = async {
        file.write_all(content).await?;
        file.sync_all().await?;
        Ok::<(), Error>(())
    }
    .await;
    drop(file);
    if let Err(error) = staged {
        let _ = remove_file(&temporary).await;
        return Err(io_error(error));
    }

    match replace_durable(&temporary, path) {
        Ok(()) => Ok(()),
        Err(DurableReplaceError::NotCommitted(error)) => {
            let _ = remove_file(&temporary).await;
            Err(io_error(error))
        }
        Err(DurableReplaceError::DurabilityUncertain(error)) => Err(PortError::Failed(format!(
            "{DURABILITY_UNCERTAIN_PREFIX}: {error}"
        ))),
    }
}

/// Synchronous equivalent for product-owner protocols that hold blocking OS
/// locks across their complete read/modify/write transition.
pub fn write_document_bytes_durable_sync(path: &Path, content: &[u8]) -> Result<(), PortError> {
    let parent = path
        .parent()
        .ok_or_else(|| PortError::Io("document path has no parent".into()))?;
    std::fs::create_dir_all(parent).map_err(io_error)?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| PortError::Io("document path has no filename".into()))?;
    let (temporary, mut file) = loop {
        let sequence = NEXT_TEMPORARY.fetch_add(1, Ordering::Relaxed);
        let temporary = parent.join(format!(".{name}.tmp-{}-{sequence}", id()));
        match StdOpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => break (temporary, file),
            Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(io_error(error)),
        }
    };
    let staged = file.write_all(content).and_then(|()| file.sync_all());
    drop(file);
    if let Err(error) = staged {
        let _ = std::fs::remove_file(&temporary);
        return Err(io_error(error));
    }

    match replace_durable(&temporary, path) {
        Ok(()) => Ok(()),
        Err(DurableReplaceError::NotCommitted(error)) => {
            let _ = std::fs::remove_file(&temporary);
            Err(io_error(error))
        }
        Err(DurableReplaceError::DurabilityUncertain(error)) => Err(PortError::Failed(format!(
            "{RECORD_DURABILITY_UNCERTAIN_PREFIX}: {error}"
        ))),
    }
}

fn io_error(error: Error) -> PortError {
    match error.kind() {
        ErrorKind::PermissionDenied => PortError::PermissionDenied(error.to_string()),
        _ => PortError::Io(error.to_string()),
    }
}

#[cfg(test)]
#[path = "document_store_tests.rs"]
mod tests;
