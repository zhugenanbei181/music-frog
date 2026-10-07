//! Whole-document sidecar persistence. Missing files alone mean no stored value.

use infiltrator_ports::error::PortError;
use std::io::{Error, ErrorKind};
use std::path::Path;
use std::process::id;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::fs::{OpenOptions, create_dir_all, read_to_string, remove_file, rename};
use tokio::io::AsyncWriteExt;

pub(crate) async fn read_optional(path: &Path) -> Result<Option<String>, PortError> {
    match read_to_string(path).await {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(io_error(error)),
    }
}

pub(crate) async fn remove_optional(path: &Path) -> Result<(), PortError> {
    match remove_file(path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io_error(error)),
    }
}

/// Each writer owns a create-new temporary. Publish only a fully flushed file;
/// never delete the destination to make replacement appear to succeed.
pub(crate) async fn write_document(path: &Path, text: &str) -> Result<(), PortError> {
    static NEXT_TEMPORARY: AtomicU64 = AtomicU64::new(0);
    let parent = path
        .parent()
        .ok_or_else(|| PortError::Io("sidecar path has no parent".into()))?;
    create_dir_all(parent).await.map_err(io_error)?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| PortError::Io("sidecar path has no filename".into()))?;
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
    let result = async {
        file.write_all(text.as_bytes()).await?;
        file.sync_all().await?;
        drop(file);
        rename(&temporary, path).await
    }
    .await;
    if result.is_err() {
        // Cleanup cannot replace the original write/publication failure.
        let _ = remove_file(&temporary).await;
    }
    result.map_err(io_error)
}

fn io_error(error: Error) -> PortError {
    match error.kind() {
        ErrorKind::PermissionDenied => PortError::PermissionDenied(error.to_string()),
        _ => PortError::Io(error.to_string()),
    }
}
