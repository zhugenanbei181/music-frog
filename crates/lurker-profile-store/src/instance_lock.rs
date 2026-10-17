//! Cross-platform single-instance lock (CORE-001).
//!
//! The GUI must never run twice against the same mihomo data directory:
//! two clients would race on the same config files and fight over the same
//! external-controller port. [`try_acquire_instance_lock`] takes an advisory
//! lock on a caller-chosen lock file (typically `instance.lock` inside the
//! data directory) and returns a guard that must be held for the whole
//! process lifetime.
//!
//! Locking is deliberately kernel-mediated so that no stale state can ever
//! block startup:
//!
//! - **Unix** uses `flock(LOCK_EX | LOCK_NB)`. The kernel drops the lock when
//!   the process exits or crashes, because the lock lives on the open file
//!   description owned by the guard.
//! - **Windows** uses a named mutex (`CreateMutexW`). The kernel destroys the
//!   mutex when the last handle is closed, including on process death. The
//!   mutex name is derived from the lock path (see [`mutex_name_for_path`])
//!   so that different data directories remain independently startable.
//!
//! This crash-safety is exactly why the lock is kept in the kernel instead
//! of a "pid file with a staleness check", which would require fragile
//! cleanup logic and could deadlock the user out of the app after a crash.

#[cfg(unix)]
mod unix_lock {
    //! Raw `open(2)`/`flock(2)` owner for the cross-process lock.
    //!
    //! The descriptor is owned by the guard: closing it (on drop or process
    //! exit) releases the kernel lock, exactly like the previous
    //! `std::fs::File`-owned descriptor. `O_CLOEXEC` keeps lock ownership out
    //! of spawned children, and the owner-only `0o600` creation mode matches
    //! the coordination-artifact permissions.

    use std::ffi::CString;
    use std::io;
    use std::path::Path;

    pub struct LockFile(i32);

    impl LockFile {
        pub(super) fn descriptor(&self) -> i32 {
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

#[cfg(unix)]
use std::path::{Path, PathBuf};
#[cfg(windows)]
use std::path::{Path, PathBuf};
#[cfg(windows)]
use std::ptr::null;
use std::result;
use std::sync::mpsc::{SyncSender, sync_channel};
use std::sync::{Mutex, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use lurker_core_ports::error::PortError;

type Result<T> = result::Result<T, PortError>;

/// Held for the process lifetime; releasing on drop.
///
/// Dropping the guard releases the underlying OS lock, and so does abnormal
/// process termination (the kernel closes handles/file descriptors). The
/// guard is intentionally not shareable across threads: on Windows,
/// `ReleaseMutex` must be called by the thread that acquired the mutex.
pub struct InstanceLockGuard {
    /// Unix: owning the descriptor keeps the `flock`; closing it (on drop or
    /// process exit) releases it.
    #[cfg(unix)]
    _lock: unix_lock::LockFile,
    /// Windows: raw handle to the `Global\...` named mutex, released in
    /// [`InstanceLockGuard::Drop`]. `HANDLE` is `*mut c_void`, so the guard
    /// is `!Send`/`!Sync` by construction, matching the owning-thread rule.
    #[cfg(windows)]
    handle: windows_sys::Win32::Foundation::HANDLE,
}

/// A transferable owner for an OS lock. The worker thread keeps the native
/// guard on the thread that acquired it, which is required by Windows mutexes;
/// dropping this value asks that thread to release the guard. Process death
/// still releases the underlying kernel handle.
pub struct ThreadedInstanceLockGuard {
    release: Mutex<Option<SyncSender<()>>>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

enum WorkerAcquisition {
    Acquired,
    Busy,
    Failed(String),
}

/// Acquire a path-scoped OS lock, waiting until another holder releases it.
/// The returned guard is Send + Sync and can span async work.
pub fn acquire_threaded_instance_lock(lock_path: &Path) -> Result<ThreadedInstanceLockGuard> {
    match spawn_threaded_lock(lock_path.to_path_buf(), true)? {
        Some(guard) => Ok(guard),
        None => Err(PortError::Failed(
            "blocking instance lock worker returned without acquiring".to_string(),
        )),
    }
}

/// Attempt to acquire a path-scoped OS lock without waiting.
pub fn try_acquire_threaded_instance_lock(
    lock_path: &Path,
) -> Result<Option<ThreadedInstanceLockGuard>> {
    spawn_threaded_lock(lock_path.to_path_buf(), false)
}

fn spawn_threaded_lock(
    lock_path: PathBuf,
    wait_until_available: bool,
) -> Result<Option<ThreadedInstanceLockGuard>> {
    let (ready_sender, ready_receiver) = sync_channel(1);
    let (release_sender, release_receiver) = sync_channel(1);
    let worker = thread::Builder::new()
        .name("music-frog-instance-lock".to_string())
        .spawn(move || {
            loop {
                match try_acquire_instance_lock(&lock_path) {
                    Ok(Some(guard)) => {
                        let _ = ready_sender.send(WorkerAcquisition::Acquired);
                        let _ = release_receiver.recv();
                        drop(guard);
                        return;
                    }
                    Ok(None) if wait_until_available => thread::sleep(Duration::from_millis(10)),
                    Ok(None) => {
                        let _ = ready_sender.send(WorkerAcquisition::Busy);
                        return;
                    }
                    Err(error) => {
                        let _ = ready_sender.send(WorkerAcquisition::Failed(error.to_string()));
                        return;
                    }
                }
            }
        })
        .map_err(|error| PortError::Failed(format!("instance lock worker: {error}")))?;

    match ready_receiver.recv() {
        Ok(WorkerAcquisition::Acquired) => Ok(Some(ThreadedInstanceLockGuard {
            release: Mutex::new(Some(release_sender)),
            worker: Mutex::new(Some(worker)),
        })),
        Ok(WorkerAcquisition::Busy) => {
            let _ = worker.join();
            Ok(None)
        }
        Ok(WorkerAcquisition::Failed(reason)) => {
            let _ = worker.join();
            Err(PortError::Failed(format!("instance lock: {reason}")))
        }
        Err(error) => {
            let _ = worker.join();
            Err(PortError::Failed(format!(
                "instance lock worker stopped before acquisition: {error}"
            )))
        }
    }
}

impl Drop for ThreadedInstanceLockGuard {
    fn drop(&mut self) {
        let release = self
            .release
            .get_mut()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        if let Some(release) = release {
            let _ = release.send(());
        }
        let worker = self
            .worker
            .get_mut()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        if let Some(worker) = worker {
            let _ = worker.join();
        }
    }
}

/// Try to become the single instance owning `lock_path`.
///
/// - `Ok(Some(guard))` — acquired; keep the guard alive for the whole
///   process lifetime.
/// - `Ok(None)` — another instance already holds the lock; the caller should
///   show an "already running" message and exit.
/// - `Err` — real failure (permission, IO). On Unix the parent directory of
///   `lock_path` must already exist.
///
/// On Windows the file at `lock_path` is never opened; the path only
/// parameterizes the named mutex name, so filesystem permissions do not
/// affect the lock there.
pub fn try_acquire_instance_lock(lock_path: &Path) -> Result<Option<InstanceLockGuard>> {
    #[cfg(unix)]
    {
        unix_try_acquire(lock_path)
    }
    #[cfg(windows)]
    {
        windows_try_acquire(lock_path)
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

/// Derive the Windows named-mutex name from the lock path.
///
/// Mutex names cannot contain path separators and the `Global\` prefix uses
/// `\` itself, so instead of sanitizing characters the full path string is
/// hashed with FNV-1a 64-bit (deterministic across runs and processes, unlike
/// `DefaultHasher`). Example: `C:\...\data\instance.lock` becomes
/// `Global\music-frog-instance-<16 hex digits>`. Two processes pointed at the
/// same data directory map to the same mutex; different data directories map
/// to distinct mutexes and can run concurrently.
#[cfg(windows)]
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

#[cfg(unix)]
fn unix_try_acquire(lock_path: &Path) -> Result<Option<InstanceLockGuard>> {
    let lock = unix_lock::try_acquire(lock_path)?;
    Ok(lock.map(|lock| InstanceLockGuard { _lock: lock }))
}

#[cfg(windows)]
fn windows_try_acquire(lock_path: &Path) -> Result<Option<InstanceLockGuard>> {
    use std::io;
    use windows_sys::Win32::Foundation;
    use windows_sys::Win32::System::Threading;

    const TRUE: i32 = 1; // windows_sys::core::BOOL

    let name = mutex_name_for_path(lock_path);
    let mut wide_name: Vec<u16> = name.encode_utf16().collect();
    wide_name.push(0); // PCWSTR is NUL-terminated.

    // CreateMutexW returns a handle even when the mutex already exists; the
    // caller distinguishes the two cases via GetLastError. With
    // bInitialOwner = TRUE the first process owns it immediately, so the
    // mutex being held is equivalent to "another instance is running".
    let handle = unsafe { Threading::CreateMutexW(null(), TRUE, wide_name.as_ptr()) };
    if handle.is_null() {
        return Err(io::Error::last_os_error().into());
    }

    let already_exists = unsafe { Foundation::GetLastError() } == Foundation::ERROR_ALREADY_EXISTS;
    if already_exists {
        // We never became the owner, so there is nothing to release; just
        // drop our view of the existing mutex.
        unsafe { Foundation::CloseHandle(handle) };
        return Ok(None);
    }

    Ok(Some(InstanceLockGuard { handle }))
}

#[cfg(windows)]
impl Drop for InstanceLockGuard {
    fn drop(&mut self) {
        use windows_sys::Win32::Foundation;
        use windows_sys::Win32::System::Threading;

        // ReleaseMutex fails harmlessly if ownership was lost (only possible
        // when the mutex was abandoned, i.e. another handle existed); either
        // way CloseHandle is what lets the next instance start.
        unsafe {
            Threading::ReleaseMutex(self.handle);
            Foundation::CloseHandle(self.handle);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;
    use std::mem::MaybeUninit;

    #[test]
    fn first_acquire_succeeds() {
        let dir = tempfile::tempdir().unwrap();
        let lock_path = dir.path().join("instance.lock");
        let guard = try_acquire_instance_lock(&lock_path).unwrap();
        assert!(guard.is_some(), "first acquisition must succeed");
    }

    #[test]
    fn second_acquire_while_held_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let lock_path = dir.path().join("instance.lock");
        let _guard = try_acquire_instance_lock(&lock_path)
            .unwrap()
            .expect("first acquisition");
        // A second open() in the same process gets an independent file
        // description, so the kernel genuinely rejects it (flock semantics).
        let second = try_acquire_instance_lock(&lock_path).unwrap();
        assert!(second.is_none(), "second acquisition must be rejected");
    }

    #[test]
    fn reacquire_after_drop_succeeds() {
        let dir = tempfile::tempdir().unwrap();
        let lock_path = dir.path().join("instance.lock");
        drop(try_acquire_instance_lock(&lock_path).unwrap().unwrap());
        let again = try_acquire_instance_lock(&lock_path).unwrap();
        assert!(again.is_some(), "lock must be reacquirable after drop");
    }

    #[test]
    fn different_paths_lock_independently() {
        let dir = tempfile::tempdir().unwrap();
        let a = try_acquire_instance_lock(&dir.path().join("a.lock"))
            .unwrap()
            .unwrap();
        let b = try_acquire_instance_lock(&dir.path().join("b.lock"))
            .unwrap()
            .unwrap();
        // Keep both alive to prove they coexist.
        let _ = (&a, &b);
    }

    #[cfg(unix)]
    #[test]
    fn lock_file_has_owner_only_permissions() {
        let dir = tempfile::tempdir().unwrap();
        let lock_path = dir.path().join("instance.lock");
        let _guard = try_acquire_instance_lock(&lock_path).unwrap().unwrap();
        let path = CString::new(lock_path.as_os_str().as_encoded_bytes()).unwrap();
        let mut metadata = MaybeUninit::<libc::stat>::uninit();
        let result = unsafe { libc::stat(path.as_ptr(), metadata.as_mut_ptr()) };
        assert_eq!(result, 0, "stat of the lock file must succeed");
        let metadata = unsafe { metadata.assume_init() };
        assert_eq!(
            metadata.st_mode & 0o777,
            0o600,
            "lock file must be owner-only"
        );
    }

    #[cfg(windows)]
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
