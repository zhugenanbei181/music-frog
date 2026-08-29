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

use std::path::Path;

use mihomo_api::error::Result;

/// Held for the process lifetime; releasing on drop.
///
/// Dropping the guard releases the underlying OS lock, and so does abnormal
/// process termination (the kernel closes handles/file descriptors). The
/// guard is intentionally not shareable across threads: on Windows,
/// `ReleaseMutex` must be called by the thread that acquired the mutex.
pub struct InstanceLockGuard {
    /// Unix: owning the `File` keeps the fd open; closing it (on drop or
    /// process exit) releases the `flock`.
    #[cfg(unix)]
    _file: std::fs::File,
    /// Windows: raw handle to the `Global\...` named mutex, released in
    /// [`InstanceLockGuard::Drop`]. `HANDLE` is `*mut c_void`, so the guard
    /// is `!Send`/`!Sync` by construction, matching the owning-thread rule.
    #[cfg(windows)]
    handle: windows_sys::Win32::Foundation::HANDLE,
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
        Err(mihomo_api::error::MihomoError::Service(
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
    use std::io::ErrorKind;
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::OpenOptionsExt;

    // Owner-only permissions: the file is a coordination artifact, not data.
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(lock_path)?;

    // Non-blocking exclusive lock; returns immediately instead of waiting,
    // which is what turns "already running" into a fast, silent bail-out.
    let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if rc == 0 {
        return Ok(Some(InstanceLockGuard { _file: file }));
    }

    let err = std::io::Error::last_os_error();
    // EWOULDBLOCK == EAGAIN on every supported unix target (Linux, macOS,
    // Android), so matching both would trip `unreachable_patterns`.
    if err.kind() == ErrorKind::WouldBlock || err.raw_os_error() == Some(libc::EWOULDBLOCK) {
        Ok(None)
    } else {
        Err(err.into())
    }
}

#[cfg(windows)]
fn windows_try_acquire(lock_path: &Path) -> Result<Option<InstanceLockGuard>> {
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
    let handle =
        unsafe { Threading::CreateMutexW(std::ptr::null(), TRUE, wide_name.as_ptr()) };
    if handle.is_null() {
        return Err(std::io::Error::last_os_error().into());
    }

    let already_exists =
        unsafe { Foundation::GetLastError() } == Foundation::ERROR_ALREADY_EXISTS;
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
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let lock_path = dir.path().join("instance.lock");
        let _guard = try_acquire_instance_lock(&lock_path).unwrap().unwrap();
        let mode = std::fs::metadata(&lock_path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "lock file must be owner-only");
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
