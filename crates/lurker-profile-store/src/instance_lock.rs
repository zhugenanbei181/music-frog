//! Cross-platform single-instance lock (CORE-001).
//!
//! The GUI must never run twice against the same mihomo data directory:
//! two clients would race on the same config files and fight over the same
//! external-controller port. [`try_acquire_threaded_instance_lock`] takes an
//! advisory lock on a caller-chosen lock file (typically `instance.lock` inside
//! the data directory) and returns a guard that must be held for the whole
//! process lifetime.
//!
//! Locking is deliberately kernel-mediated so that no stale state can ever
//! block startup: the kernel drops the lock when the process exits or crashes.
//! The actual per-OS primitive (`flock` on Unix, a named mutex on Windows) is
//! owned by [`crate::platform::NativeStorePlatform`]; this module only owns the
//! portable worker-thread hand-off that keeps a thread-affine Windows mutex on
//! the thread that acquired it.
//!
//! This crash-safety is exactly why the lock is kept in the kernel instead of a
//! "pid file with a staleness check", which would require fragile cleanup logic
//! and could deadlock the user out of the app after a crash.

use crate::platform::{NativeStorePlatform, StorePlatform};
use std::path::{Path, PathBuf};
use std::result;
use std::sync::mpsc::{SyncSender, sync_channel};
use std::sync::{Mutex, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use lurker_core_ports::error::PortError;

type Result<T> = result::Result<T, PortError>;

/// One held OS lock.
///
/// Dropping the guard releases the underlying kernel lock, and so does abnormal
/// process termination (the kernel closes handles/file descriptors). The fence
/// is intentionally not required to be `Send`: on Windows `ReleaseMutex` must
/// be called by the thread that acquired the mutex, which is why
/// [`ThreadedInstanceLockGuard`] keeps it on its worker thread.
pub trait InstanceLockFence {
    /// Release the lock. Consuming the guard is the only release path.
    fn unlock(self: Box<Self>) -> result::Result<(), PortError>;
}

/// A held lock returned by [`try_acquire_instance_lock`].
pub type InstanceLockGuard = Box<dyn InstanceLockFence>;

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

/// Try to become the single instance owning `lock_path` through the native
/// platform.
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
    NativeStorePlatform.try_acquire_instance_lock(lock_path)
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
}
