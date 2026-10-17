//! Durable replacement for small state documents.
//!
//! Callers must flush the staged file before invoking this function. Both
//! paths must be siblings so replacement stays on one filesystem. The per-OS
//! syscall path lives behind [`crate::platform::NativeStorePlatform`]; these
//! free functions are the native default used by callers that do not own a
//! [`crate::platform::StorePlatform`].

use crate::platform::{NativeStorePlatform, StorePlatform};
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::io;
use std::path::Path;

/// Failure stage matters because a directory flush may fail after the new
/// destination is already visible. Callers must preserve recovery evidence in
/// that case and must not report the write as durably committed.
#[derive(Debug)]
pub enum DurableReplaceError {
    NotCommitted(io::Error),
    DurabilityUncertain(io::Error),
}

impl Display for DurableReplaceError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotCommitted(error) => write!(formatter, "atomic replacement failed: {error}"),
            Self::DurabilityUncertain(error) => {
                write!(
                    formatter,
                    "replacement succeeded but durability is uncertain: {error}"
                )
            }
        }
    }
}

impl Error for DurableReplaceError {}

/// Atomically replace `destination` with the already-flushed `staged` file and
/// request the strongest supported persistence guarantee of the native
/// platform.
pub fn replace_durable(staged: &Path, destination: &Path) -> Result<(), DurableReplaceError> {
    NativeStorePlatform.replace_atomic(staged, destination)
}

/// Remove a file and request durable persistence of the directory entry with
/// the native platform. A failure after unlink is reported as uncertain because
/// the file may already be absent even though that absence was not flushed.
pub fn remove_durable(path: &Path) -> Result<(), DurableReplaceError> {
    NativeStorePlatform.remove_file_durable(path)
}

#[cfg(test)]
mod tests {
    use super::{DurableReplaceError, remove_durable, replace_durable};
    use std::fs::{self, File};

    #[test]
    fn replaces_an_existing_document_after_staged_file_flush() {
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("state.json");
        let staged = directory.path().join(".state.json.tmp");
        fs::write(&destination, b"old").unwrap();
        fs::write(&staged, b"new").unwrap();
        File::options()
            .write(true)
            .open(&staged)
            .unwrap()
            .sync_all()
            .unwrap();

        replace_durable(&staged, &destination).unwrap();

        assert_eq!(fs::read(destination).unwrap(), b"new");
        assert!(!staged.exists());
    }

    #[test]
    fn durable_remove_reports_uncertain_directory_flush_instead_of_succeeding() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("state.json");
        fs::write(&path, b"state").unwrap();

        match remove_durable(&path) {
            Ok(()) => assert!(!path.exists()),
            Err(DurableReplaceError::DurabilityUncertain(_)) => assert!(!path.exists()),
            Err(DurableReplaceError::NotCommitted(error)) => {
                panic!("durable removal failed before unlink: {error}")
            }
        }
    }
}
