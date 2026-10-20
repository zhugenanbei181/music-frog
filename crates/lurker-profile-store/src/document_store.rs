//! Durable whole-document and sidecar operations shared by engine stores.
//!
//! Every byte-level operation is delegated to the store's
//! [`StorePlatform`]; the free functions in this module are the native default
//! used by callers that do not own a platform handle.

use crate::durable_replace::DurableReplaceError;
use crate::platform::{
    NativeStorePlatform, StorePlatform, map_io_error, native_platform, run_platform_io,
    run_platform_replace,
};
use lurker_core_ports::error::PortError;
use std::path::Path;
use std::sync::Arc;

const DURABILITY_UNCERTAIN_PREFIX: &str =
    "document replacement succeeded but directory durability is uncertain";
const REMOVAL_DURABILITY_UNCERTAIN_PREFIX: &str =
    "document removal succeeded but directory durability is uncertain";
/// The message prefix of a failure whose record replacement is visible but
/// whose directory durability could not be proven.
pub const RECORD_DURABILITY_UNCERTAIN_PREFIX: &str =
    "record rename succeeded but directory durability is uncertain";

/// Missing files alone are represented as `None`; permission and I/O errors stay explicit.
pub async fn read_optional(path: &Path) -> Result<Option<String>, PortError> {
    let platform = native_platform();
    read_optional_with_platform(&platform, path).await
}

/// [`read_optional`] through an injected platform.
pub async fn read_optional_with_platform(
    platform: &Arc<dyn StorePlatform>,
    path: &Path,
) -> Result<Option<String>, PortError> {
    let Some(bytes) = read_optional_bytes_with_platform(platform, path).await? else {
        return Ok(None);
    };
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|error| PortError::Io(error.to_string()))
}

/// Read the exact persisted bytes without interpreting their engine format.
pub async fn read_optional_bytes(path: &Path) -> Result<Option<Vec<u8>>, PortError> {
    let platform = native_platform();
    read_optional_bytes_with_platform(&platform, path).await
}

/// [`read_optional_bytes`] through an injected platform.
pub async fn read_optional_bytes_with_platform(
    platform: &Arc<dyn StorePlatform>,
    path: &Path,
) -> Result<Option<Vec<u8>>, PortError> {
    let path = path.to_path_buf();
    run_platform_io(platform, move |platform| platform.read(&path)).await
}

/// Read bytes that must already exist; a missing file is `PortError::Io` so
/// callers keep the historical retryable storage classification.
pub async fn read_required_bytes_with_platform(
    platform: &Arc<dyn StorePlatform>,
    path: &Path,
) -> Result<Vec<u8>, PortError> {
    read_optional_bytes_with_platform(platform, path)
        .await?
        .ok_or_else(|| PortError::Io("profile document is missing".into()))
}

/// Remove an optional document. A missing file is already the requested state.
pub async fn remove_optional(path: &Path) -> Result<(), PortError> {
    let platform = native_platform();
    remove_optional_with_platform(&platform, path).await
}

/// [`remove_optional`] through an injected platform.
pub async fn remove_optional_with_platform(
    platform: &Arc<dyn StorePlatform>,
    path: &Path,
) -> Result<(), PortError> {
    let path = path.to_path_buf();
    run_platform_io(platform, move |platform| platform.remove_file(&path)).await
}

/// Remove an optional document and durably publish the directory change.
pub fn remove_optional_durable(path: &Path) -> Result<(), PortError> {
    match NativeStorePlatform.remove_file_durable(path) {
        Ok(()) => Ok(()),
        Err(DurableReplaceError::NotCommitted(error)) => Err(map_io_error(error)),
        Err(DurableReplaceError::DurabilityUncertain(error)) => Err(PortError::Failed(format!(
            "{REMOVAL_DURABILITY_UNCERTAIN_PREFIX}: {error}"
        ))),
    }
}

/// Replace a UTF-8 document through the same durable byte path used by workspaces.
pub async fn write_document(path: &Path, text: &str) -> Result<(), PortError> {
    write_document_bytes(path, text.as_bytes()).await
}

/// [`write_document`] through an injected platform.
pub async fn write_document_with_platform(
    platform: &Arc<dyn StorePlatform>,
    path: &Path,
    text: &str,
) -> Result<(), PortError> {
    write_document_bytes_with_platform(platform, path, text.as_bytes()).await
}

/// Replace opaque document bytes through a fully flushed same-directory temporary.
pub async fn write_document_bytes(path: &Path, content: &[u8]) -> Result<(), PortError> {
    write_document_bytes_durable(path, content).await
}

/// [`write_document_bytes`] through an injected platform.
pub async fn write_document_bytes_with_platform(
    platform: &Arc<dyn StorePlatform>,
    path: &Path,
    content: &[u8],
) -> Result<(), PortError> {
    write_document_bytes_durable_with_platform(platform, path, content).await
}

/// Flush a staged file and use the platform's durable atomic replacement.
pub async fn write_document_bytes_durable(path: &Path, content: &[u8]) -> Result<(), PortError> {
    let platform = native_platform();
    write_document_bytes_durable_with_platform(&platform, path, content).await
}

/// [`write_document_bytes_durable`] through an injected platform.
pub async fn write_document_bytes_durable_with_platform(
    platform: &Arc<dyn StorePlatform>,
    path: &Path,
    content: &[u8],
) -> Result<(), PortError> {
    let path = path.to_path_buf();
    let content = content.to_vec();
    match run_platform_replace(platform, move |platform| {
        platform.write_atomic(&path, &content)
    })
    .await
    {
        Ok(()) => Ok(()),
        Err(DurableReplaceError::NotCommitted(error)) => Err(map_io_error(error)),
        Err(DurableReplaceError::DurabilityUncertain(error)) => Err(PortError::Failed(format!(
            "{DURABILITY_UNCERTAIN_PREFIX}: {error}"
        ))),
    }
}

/// Synchronous equivalent for product-owner protocols that hold blocking OS
/// locks across their complete read/modify/write transition.
pub fn write_document_bytes_durable_sync(path: &Path, content: &[u8]) -> Result<(), PortError> {
    match NativeStorePlatform.write_atomic(path, content) {
        Ok(()) => Ok(()),
        Err(DurableReplaceError::NotCommitted(error)) => Err(map_io_error(error)),
        Err(DurableReplaceError::DurabilityUncertain(error)) => Err(PortError::Failed(format!(
            "{RECORD_DURABILITY_UNCERTAIN_PREFIX}: {error}"
        ))),
    }
}

#[cfg(test)]
#[path = "document_store_tests.rs"]
mod tests;
