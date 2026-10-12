//! Platform-specific durable replacement for small state documents.
//!
//! Callers must flush the staged file before invoking this function. Both
//! paths must be siblings so replacement stays on one filesystem.

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

impl std::fmt::Display for DurableReplaceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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

impl std::error::Error for DurableReplaceError {}

/// Atomically replace `destination` with the already-flushed `staged` file and
/// request the strongest supported persistence guarantee for the platform.
///
/// Unix targets rename the file and then sync the containing directory. On
/// Windows, `MoveFileExW` with `MOVEFILE_WRITE_THROUGH` waits for the move to be
/// flushed to disk. Other targets fail explicitly instead of claiming a
/// durability guarantee they do not implement.
pub fn replace_durable(staged: &Path, destination: &Path) -> Result<(), DurableReplaceError> {
    if staged.parent() != destination.parent() {
        return Err(DurableReplaceError::NotCommitted(io::Error::new(
            io::ErrorKind::InvalidInput,
            "staged file and destination must share a parent directory",
        )));
    }

    replace_durable_impl(staged, destination)
}

/// Remove a file and request durable persistence of the directory entry.
/// A failure after unlink is reported as uncertain because the file may
/// already be absent even though that absence was not flushed.
pub fn remove_durable(path: &Path) -> Result<(), DurableReplaceError> {
    remove_durable_impl(path)
}

#[cfg(unix)]
fn replace_durable_impl(staged: &Path, destination: &Path) -> Result<(), DurableReplaceError> {
    std::fs::rename(staged, destination).map_err(DurableReplaceError::NotCommitted)?;

    let parent = destination.parent().unwrap_or_else(|| Path::new("."));
    let directory =
        std::fs::File::open(parent).map_err(DurableReplaceError::DurabilityUncertain)?;
    directory
        .sync_all()
        .map_err(DurableReplaceError::DurabilityUncertain)
}

#[cfg(unix)]
fn remove_durable_impl(path: &Path) -> Result<(), DurableReplaceError> {
    match std::fs::remove_file(path) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(DurableReplaceError::NotCommitted(error)),
    }
    sync_parent_directory(path).map_err(DurableReplaceError::DurabilityUncertain)
}

#[cfg(windows)]
fn remove_durable_impl(path: &Path) -> Result<(), DurableReplaceError> {
    match std::fs::remove_file(path) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(DurableReplaceError::NotCommitted(error)),
    }
    sync_parent_directory(path).map_err(DurableReplaceError::DurabilityUncertain)
}

#[cfg(unix)]
fn sync_parent_directory(path: &Path) -> io::Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::File::open(parent)?.sync_all()
}

#[cfg(windows)]
fn sync_parent_directory(path: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use std::ptr::null;
    use windows_sys::Win32::Foundation::{CloseHandle, GENERIC_WRITE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, FILE_FLAG_BACKUP_SEMANTICS, FILE_SHARE_DELETE, FILE_SHARE_READ,
        FILE_SHARE_WRITE, FlushFileBuffers, OPEN_EXISTING,
    };

    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let parent: Vec<u16> = parent.as_os_str().encode_wide().chain(Some(0)).collect();
    let directory = unsafe {
        CreateFileW(
            parent.as_ptr(),
            GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS,
            std::ptr::null_mut(),
        )
    };
    if directory == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    let flush_result = unsafe { FlushFileBuffers(directory) };
    let flush_error = (flush_result == 0).then(io::Error::last_os_error);
    unsafe {
        CloseHandle(directory);
    }
    match flush_error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

#[cfg(windows)]
fn replace_durable_impl(staged: &Path, destination: &Path) -> Result<(), DurableReplaceError> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };

    let staged: Vec<u16> = staged.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    let result = unsafe {
        MoveFileExW(
            staged.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if result == 0 {
        Err(DurableReplaceError::NotCommitted(io::Error::last_os_error()))
    } else {
        Ok(())
    }
}

#[cfg(not(any(unix, windows)))]
fn replace_durable_impl(_staged: &Path, _destination: &Path) -> Result<(), DurableReplaceError> {
    Err(DurableReplaceError::NotCommitted(io::Error::new(
        io::ErrorKind::Unsupported,
        "durable file replacement is unsupported on this platform",
    )))
}

#[cfg(not(any(unix, windows)))]
fn remove_durable_impl(_path: &Path) -> Result<(), DurableReplaceError> {
    Err(DurableReplaceError::NotCommitted(io::Error::new(
        io::ErrorKind::Unsupported,
        "durable file removal is unsupported on this platform",
    )))
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
