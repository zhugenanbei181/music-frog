//! Portable platform extension point for the profile-store kernel.
//!
//! Every filesystem and kernel-lock primitive the kernel needs is expressed by
//! [`StorePlatform`]. The kernel never selects an implementation with `cfg`;
//! it stores an `Arc<dyn StorePlatform>` supplied by its owner.
//!
//! [`NativeStorePlatform`] is the only implementation that contains per-OS
//! code: Unix `flock`/directory `fsync` and the Windows `MoveFileExW` /
//! `FlushFileBuffers` / named-mutex paths. Tests and embedded hosts substitute
//! their own implementation through
//! [`crate::store::ProfileStoreCore::with_platform`].
//!
//! The byte primitives are synchronous because the kernel's durable operations
//! are synchronous; asynchronous callers run them on the blocking pool through
//! the crate's blocking/async boundary. The profile-write fence is the one
//! asynchronous method: it awaits the process-local `tokio` mutex after the
//! blocking cross-process acquisition.

use crate::durable_replace::DurableReplaceError;
use crate::file_lock::{ProfileWriteFence, lock_profile_writes_native};
use crate::instance_lock::InstanceLockFence;
use crate::layout::ProfileStoreLayout;
use lurker_core_ports::error::PortError;
use std::fs::{self, OpenOptions};
use std::io::{self, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::process::id;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::task::{JoinError, spawn_blocking};

/// Suffix source for same-directory staged temporaries.
static NEXT_TEMPORARY: AtomicU64 = AtomicU64::new(0);

fn next_temporary_sequence() -> u64 {
    NEXT_TEMPORARY.fetch_add(1, Ordering::Relaxed)
}

/// One directory entry as seen by the kernel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoreDirEntry {
    /// The absolute path of the entry.
    pub path: PathBuf,
    /// The kind of the entry.
    pub kind: StoreEntryKind,
}

/// Kind of one directory entry. Anything that is neither a file nor a
/// directory stays explicit so migrations can refuse it instead of guessing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StoreEntryKind {
    /// A regular file.
    File,
    /// A directory.
    Directory,
    /// Anything else, for example a symlink or a device node.
    Other,
}

/// Filesystem and kernel-lock primitives owned by one store.
///
/// Implementations must be safe to share across threads: the kernel clones the
/// `Arc` into blocking workers.
#[async_trait::async_trait]
pub trait StorePlatform: Send + Sync {
    /// Exact bytes of `path`; a missing file is `None`, every other error stays
    /// explicit.
    fn read(&self, path: &Path) -> Result<Option<Vec<u8>>, PortError>;

    /// Durably replace `path` with `bytes` through a fully flushed sibling
    /// temporary. A failure after the new file became visible is
    /// [`DurableReplaceError::DurabilityUncertain`]; callers must not report
    /// that as committed.
    fn write_atomic(&self, path: &Path, bytes: &[u8]) -> Result<(), DurableReplaceError>;

    /// Durably replace `target` with the already-flushed `source` file. Both
    /// paths must be siblings so the replacement stays on one filesystem.
    fn replace_atomic(&self, source: &Path, target: &Path) -> Result<(), DurableReplaceError>;

    /// Remove `path`; an already missing file is the requested state.
    fn remove_file(&self, path: &Path) -> Result<(), PortError>;

    /// Remove `path` and request durable persistence of the directory entry.
    fn remove_file_durable(&self, path: &Path) -> Result<(), DurableReplaceError>;

    /// Move `source` to `target` without any durability claim.
    fn rename(&self, source: &Path, target: &Path) -> Result<(), PortError>;

    /// Create `path` and every missing parent directory.
    fn create_dir_all(&self, path: &Path) -> Result<(), PortError>;

    /// Remove one empty directory.
    fn remove_dir(&self, path: &Path) -> Result<(), PortError>;

    /// Whether `path` exists.
    fn exists(&self, path: &Path) -> Result<bool, PortError>;

    /// Canonical absolute path. Must report [`PortError::NotFound`] for a
    /// missing path so callers keep distinguishing absence from I/O failure.
    fn canonicalize(&self, path: &Path) -> Result<PathBuf, PortError>;

    /// Read the entries of one directory.
    fn read_dir(&self, path: &Path) -> Result<Vec<StoreDirEntry>, PortError>;

    /// Acquire the shared engine write fence in process-local and cross-process
    /// order. Every profile read/write path must hold it.
    async fn lock_profile_writes(
        &self,
        layout: &ProfileStoreLayout,
        config_dir: &Path,
    ) -> Result<Box<dyn ProfileWriteFence>, PortError>;

    /// Attempt a path-scoped exclusive kernel lock without waiting.
    ///
    /// `Ok(None)` means another holder currently owns the lock; that is the
    /// normal contention result, not an error.
    fn try_acquire_instance_lock(
        &self,
        path: &Path,
    ) -> Result<Option<Box<dyn InstanceLockFence>>, PortError>;
}

/// The kernel's platform: the host filesystem plus the real OS locks.
///
/// This is the only production type in the crate with `cfg(unix)` /
/// `cfg(windows)` / `libc` / `windows-sys` code.
pub struct NativeStorePlatform;

impl NativeStorePlatform {
    /// Creates the native platform.
    pub const fn new() -> Self {
        Self
    }
}

impl Default for NativeStorePlatform {
    fn default() -> Self {
        Self
    }
}

/// The native platform as a shared trait object.
pub fn native_platform() -> Arc<dyn StorePlatform> {
    Arc::new(NativeStorePlatform)
}

#[async_trait::async_trait]
impl StorePlatform for NativeStorePlatform {
    fn read(&self, path: &Path) -> Result<Option<Vec<u8>>, PortError> {
        match fs::read(path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
            Err(error) => Err(map_io_error(error)),
        }
    }

    fn write_atomic(&self, path: &Path, bytes: &[u8]) -> Result<(), DurableReplaceError> {
        let parent = path.parent().ok_or_else(|| {
            DurableReplaceError::NotCommitted(io::Error::new(
                ErrorKind::InvalidInput,
                "document path has no parent",
            ))
        })?;
        fs::create_dir_all(parent).map_err(DurableReplaceError::NotCommitted)?;
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                DurableReplaceError::NotCommitted(io::Error::new(
                    ErrorKind::InvalidInput,
                    "document path has no filename",
                ))
            })?;
        let (temporary, mut file) = loop {
            let sequence = next_temporary_sequence();
            let temporary = parent.join(format!(".{name}.tmp-{}-{sequence}", id()));
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)
            {
                Ok(file) => break (temporary, file),
                Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(DurableReplaceError::NotCommitted(error)),
            }
        };
        let staged = file.write_all(bytes).and_then(|()| file.sync_all());
        drop(file);
        if let Err(error) = staged {
            let _ = fs::remove_file(&temporary);
            return Err(DurableReplaceError::NotCommitted(error));
        }

        match self.replace_atomic(&temporary, path) {
            Ok(()) => Ok(()),
            Err(error) => {
                let _ = fs::remove_file(&temporary);
                Err(error)
            }
        }
    }

    fn replace_atomic(&self, source: &Path, target: &Path) -> Result<(), DurableReplaceError> {
        replace_durable_impl(source, target)
    }

    fn remove_file(&self, path: &Path) -> Result<(), PortError> {
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
            Err(error) => Err(map_io_error(error)),
        }
    }

    fn remove_file_durable(&self, path: &Path) -> Result<(), DurableReplaceError> {
        remove_durable_impl(path)
    }

    fn rename(&self, source: &Path, target: &Path) -> Result<(), PortError> {
        fs::rename(source, target).map_err(map_io_error)
    }

    fn create_dir_all(&self, path: &Path) -> Result<(), PortError> {
        fs::create_dir_all(path).map_err(map_io_error)
    }

    fn remove_dir(&self, path: &Path) -> Result<(), PortError> {
        fs::remove_dir(path).map_err(map_io_error)
    }

    fn exists(&self, path: &Path) -> Result<bool, PortError> {
        path.try_exists().map_err(map_io_error)
    }

    fn canonicalize(&self, path: &Path) -> Result<PathBuf, PortError> {
        fs::canonicalize(path).map_err(map_io_error)
    }

    fn read_dir(&self, path: &Path) -> Result<Vec<StoreDirEntry>, PortError> {
        let entries = fs::read_dir(path).map_err(map_io_error)?;
        let mut collected = Vec::new();
        for entry in entries {
            let entry = entry.map_err(map_io_error)?;
            let file_type = entry.file_type().map_err(map_io_error)?;
            let kind = if file_type.is_dir() {
                StoreEntryKind::Directory
            } else if file_type.is_file() {
                StoreEntryKind::File
            } else {
                StoreEntryKind::Other
            };
            collected.push(StoreDirEntry {
                path: entry.path(),
                kind,
            });
        }
        Ok(collected)
    }

    async fn lock_profile_writes(
        &self,
        layout: &ProfileStoreLayout,
        config_dir: &Path,
    ) -> Result<Box<dyn ProfileWriteFence>, PortError> {
        lock_profile_writes_native(layout, config_dir).await
    }

    fn try_acquire_instance_lock(
        &self,
        path: &Path,
    ) -> Result<Option<Box<dyn InstanceLockFence>>, PortError> {
        native_try_acquire_instance_lock(path)
    }
}

/// Run one synchronous platform operation on the blocking pool.
pub(crate) async fn run_platform_io<T, F>(
    platform: &Arc<dyn StorePlatform>,
    operation: F,
) -> Result<T, PortError>
where
    T: Send + 'static,
    F: FnOnce(&dyn StorePlatform) -> Result<T, PortError> + Send + 'static,
{
    let platform = Arc::clone(platform);
    spawn_blocking(move || operation(platform.as_ref()))
        .await
        .map_err(platform_worker_error)?
}

/// Run one synchronous replacement on the blocking pool, keeping the
/// `NotCommitted` / `DurabilityUncertain` split intact.
pub(crate) async fn run_platform_replace<T, F>(
    platform: &Arc<dyn StorePlatform>,
    operation: F,
) -> Result<T, DurableReplaceError>
where
    T: Send + 'static,
    F: FnOnce(&dyn StorePlatform) -> Result<T, DurableReplaceError> + Send + 'static,
{
    let platform = Arc::clone(platform);
    spawn_blocking(move || operation(platform.as_ref()))
        .await
        .map_err(|error| {
            DurableReplaceError::NotCommitted(io::Error::other(format!(
                "profile-store platform worker: {error}"
            )))
        })?
}

fn platform_worker_error(error: JoinError) -> PortError {
    PortError::Failed(format!("profile-store platform worker: {error}"))
}

pub(crate) fn map_io_error(error: io::Error) -> PortError {
    match error.kind() {
        ErrorKind::PermissionDenied => PortError::PermissionDenied(error.to_string()),
        ErrorKind::NotFound => PortError::NotFound(error.to_string()),
        _ => PortError::Io(error.to_string()),
    }
}

fn native_try_acquire_instance_lock(
    lock_path: &Path,
) -> Result<Option<Box<dyn InstanceLockFence>>, PortError> {
    #[cfg(unix)]
    {
        let lock = unix_lock::try_acquire(lock_path).map_err(map_io_error)?;
        Ok(lock
            .map(|lock| Box::new(NativeInstanceLock { _lock: lock }) as Box<dyn InstanceLockFence>))
    }
    #[cfg(windows)]
    {
        windows_lock::try_acquire(lock_path)
    }
    // Other targets have no implementation and always fail closed.
    #[cfg(not(any(unix, windows)))]
    {
        let _ = lock_path;
        Err(PortError::Failed(
            "instance lock: unsupported platform".to_string(),
        ))
    }
}

#[cfg(unix)]
struct NativeInstanceLock {
    _lock: unix_lock::LockFile,
}

#[cfg(unix)]
impl InstanceLockFence for NativeInstanceLock {
    fn unlock(self: Box<Self>) -> Result<(), PortError> {
        drop(self);
        Ok(())
    }
}

#[cfg(unix)]
mod unix_lock {
    //! Raw `open(2)`/`flock(2)` owner for the cross-process lock.
    //!
    //! The descriptor is owned by the guard: closing it (on drop or process
    //! exit) releases the kernel lock. `O_CLOEXEC` keeps lock ownership out of
    //! spawned children, and the owner-only `0o600` creation mode matches the
    //! coordination-artifact permissions.

    use std::ffi::CString;
    use std::io;
    use std::path::Path;

    pub struct LockFile(i32);

    impl LockFile {
        fn descriptor(&self) -> i32 {
            self.0
        }
    }

    impl Drop for LockFile {
        fn drop(&mut self) {
            unsafe {
                libc::close(self.0);
            }
        }
    }

    pub(super) fn try_acquire(lock_path: &Path) -> io::Result<Option<LockFile>> {
        let path = CString::new(lock_path.as_os_str().as_encoded_bytes()).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "lock path contains an interior NUL byte",
            )
        })?;
        let descriptor = unsafe {
            libc::open(
                path.as_ptr(),
                libc::O_RDWR | libc::O_CREAT | libc::O_CLOEXEC,
                0o600 as libc::c_uint,
            )
        };
        if descriptor < 0 {
            return Err(io::Error::last_os_error());
        }
        let lock = LockFile(descriptor);
        let result = unsafe { libc::flock(lock.descriptor(), libc::LOCK_EX | libc::LOCK_NB) };
        if result == 0 {
            return Ok(Some(lock));
        }
        let error = io::Error::last_os_error();
        // EWOULDBLOCK == EAGAIN on every supported unix target (Linux, macOS,
        // Android, iOS), so matching both would trip `unreachable_patterns`.
        if error.kind() == io::ErrorKind::WouldBlock
            || error.raw_os_error() == Some(libc::EWOULDBLOCK)
        {
            Ok(None)
        } else {
            Err(error)
        }
    }
}

#[cfg(windows)]
mod windows_lock {
    //! Named-mutex owner for the cross-process lock.
    //!
    //! `CreateMutexW` returns a handle even when the mutex already exists; the
    //! caller distinguishes the two cases via `GetLastError`. With
    //! `bInitialOwner = TRUE` the first process owns it immediately, so the
    //! mutex being held is equivalent to "another instance is running". The
    //! kernel destroys the mutex when the last handle is closed, including on
    //! process death.

    use super::{InstanceLockFence, map_io_error};
    use lurker_core_ports::error::PortError;
    use std::io;
    use std::path::Path;
    use std::ptr::null;
    use windows_sys::Win32::Foundation;
    use windows_sys::Win32::System::Threading;

    const TRUE: i32 = 1; // windows_sys::core::BOOL

    /// Raw handle to the `Global\...` named mutex, released in
    /// [`NativeInstanceLock::drop`]. `HANDLE` is `*mut c_void`, so the guard is
    /// `!Send`/`!Sync` by construction, matching the owning-thread rule.
    pub(super) struct NativeInstanceLock(Foundation::HANDLE);

    impl InstanceLockFence for NativeInstanceLock {
        fn unlock(self: Box<Self>) -> Result<(), PortError> {
            drop(self);
            Ok(())
        }
    }

    pub(super) fn try_acquire(
        lock_path: &Path,
    ) -> Result<Option<Box<dyn InstanceLockFence>>, PortError> {
        let name = mutex_name_for_path(lock_path);
        let mut wide_name: Vec<u16> = name.encode_utf16().collect();
        wide_name.push(0); // PCWSTR is NUL-terminated.

        let handle = unsafe { Threading::CreateMutexW(null(), TRUE, wide_name.as_ptr()) };
        if handle.is_null() {
            return Err(map_io_error(io::Error::last_os_error()));
        }

        let already_exists =
            unsafe { Foundation::GetLastError() } == Foundation::ERROR_ALREADY_EXISTS;
        if already_exists {
            // We never became the owner, so there is nothing to release; just
            // drop our view of the existing mutex.
            unsafe { Foundation::CloseHandle(handle) };
            return Ok(None);
        }

        Ok(Some(Box::new(NativeInstanceLock(handle))))
    }

    impl Drop for NativeInstanceLock {
        fn drop(&mut self) {
            // ReleaseMutex fails harmlessly if ownership was lost (only
            // possible when the mutex was abandoned, i.e. another handle
            // existed); either way CloseHandle is what lets the next instance
            // start.
            unsafe {
                Threading::ReleaseMutex(self.0);
                Foundation::CloseHandle(self.0);
            }
        }
    }

    /// Derive the Windows named-mutex name from the lock path.
    ///
    /// Mutex names cannot contain path separators and the `Global\` prefix uses
    /// `\` itself, so instead of sanitizing characters the full path string is
    /// hashed with FNV-1a 64-bit (deterministic across runs and processes,
    /// unlike `DefaultHasher`). Two processes pointed at the same data
    /// directory map to the same mutex; different data directories map to
    /// distinct mutexes and can run concurrently.
    pub fn mutex_name_for_path(lock_path: &Path) -> String {
        const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
        const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

        let mut hash = FNV_OFFSET_BASIS;
        for byte in lock_path.to_string_lossy().as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(FNV_PRIME);
        }
        format!("Global\\music-frog-instance-{hash:016x}")
    }
}

/// Atomically replace `destination` with the already-flushed `staged` file.
///
/// Unix targets rename the file and then sync the containing directory. On
/// Windows, `MoveFileExW` with `MOVEFILE_WRITE_THROUGH` waits for the move to be
/// flushed to disk. Other targets fail explicitly instead of claiming a
/// durability guarantee they do not implement.
fn replace_durable_impl(staged: &Path, destination: &Path) -> Result<(), DurableReplaceError> {
    if staged.parent() != destination.parent() {
        return Err(DurableReplaceError::NotCommitted(io::Error::new(
            ErrorKind::InvalidInput,
            "staged file and destination must share a parent directory",
        )));
    }

    replace_durable_same_directory(staged, destination)
}

#[cfg(unix)]
fn replace_durable_same_directory(
    staged: &Path,
    destination: &Path,
) -> Result<(), DurableReplaceError> {
    use std::fs::File;

    fs::rename(staged, destination).map_err(DurableReplaceError::NotCommitted)?;

    let parent = destination.parent().unwrap_or_else(|| Path::new("."));
    let directory = File::open(parent).map_err(DurableReplaceError::DurabilityUncertain)?;
    directory
        .sync_all()
        .map_err(DurableReplaceError::DurabilityUncertain)
}

fn remove_durable_impl(path: &Path) -> Result<(), DurableReplaceError> {
    match fs::remove_file(path) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(DurableReplaceError::NotCommitted(error)),
    }
    sync_parent_directory(path).map_err(DurableReplaceError::DurabilityUncertain)
}

#[cfg(unix)]
fn sync_parent_directory(path: &Path) -> io::Result<()> {
    use std::fs::File;

    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    File::open(parent)?.sync_all()
}

#[cfg(windows)]
fn sync_parent_directory(path: &Path) -> io::Result<()> {
    use crate::wide;
    use std::ptr::{null, null_mut};
    use windows_sys::Win32::Foundation::{CloseHandle, GENERIC_WRITE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, FILE_FLAG_BACKUP_SEMANTICS, FILE_SHARE_DELETE, FILE_SHARE_READ,
        FILE_SHARE_WRITE, FlushFileBuffers, OPEN_EXISTING,
    };

    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let parent = wide::path(parent);
    let directory = unsafe {
        CreateFileW(
            parent.as_ptr(),
            GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS,
            null_mut(),
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
fn replace_durable_same_directory(
    staged: &Path,
    destination: &Path,
) -> Result<(), DurableReplaceError> {
    use crate::wide;
    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };

    let staged = wide::path(staged);
    let destination = wide::path(destination);
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
fn replace_durable_same_directory(
    _staged: &Path,
    _destination: &Path,
) -> Result<(), DurableReplaceError> {
    Err(DurableReplaceError::NotCommitted(io::Error::new(
        ErrorKind::Unsupported,
        "durable file replacement is unsupported on this platform",
    )))
}

#[cfg(not(any(unix, windows)))]
fn remove_durable_impl(_path: &Path) -> Result<(), DurableReplaceError> {
    Err(DurableReplaceError::NotCommitted(io::Error::new(
        ErrorKind::Unsupported,
        "durable file removal is unsupported on this platform",
    )))
}

#[cfg(all(test, windows))]
mod windows_tests {
    use super::windows_lock::mutex_name_for_path;
    use std::path::Path;

    #[test]
    fn mutex_name_is_deterministic_and_distinct() {
        let a = mutex_name_for_path(Path::new(r"C:\data\instance.lock"));
        let b = mutex_name_for_path(Path::new(r"C:\data\instance.lock"));
        let c = mutex_name_for_path(Path::new(r"C:\other\instance.lock"));
        assert_eq!(a, b, "same path must map to the same mutex name");
        assert_ne!(a, c, "different paths must map to different mutex names");
        assert!(a.starts_with("Global\\music-frog-instance-"));
    }
}

#[cfg(test)]
#[path = "platform_tests.rs"]
mod tests;
