//! Shared in-process and cross-process profile write fences.
//!
//! The fence itself is portable: one process-local mutex per canonical
//! directory plus the platform's cross-process lock. The native platform owns
//! the OS lock; products and tests can substitute their own through
//! [`crate::platform::StorePlatform::lock_profile_writes`].

use crate::instance_lock::{ThreadedInstanceLockGuard, try_acquire_threaded_instance_lock};
use crate::layout::ProfileStoreLayout;
use crate::platform::{NativeStorePlatform, StorePlatform, map_io_error};
use lurker_core_ports::error::PortError;
use std::collections::HashMap;
use std::env::current_dir;
use std::fs::canonicalize;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::Duration;
use tokio::sync::{self, OwnedMutexGuard};
use tokio::task::spawn_blocking;
use tokio::time::sleep;

/// The platform-owned half of a held profile-write fence.
pub trait ProfileWriteFence: Send + Sync {
    /// Release the fence. Consuming the handle is the only release path.
    fn unlock(self: Box<Self>) -> Result<(), PortError>;
}

/// A held lock that fences profile reads and writes for one engine namespace.
pub struct ProfileWriteGuard {
    fence: Option<Box<dyn ProfileWriteFence>>,
}

impl ProfileWriteGuard {
    pub(crate) fn new(fence: Box<dyn ProfileWriteFence>) -> Self {
        Self { fence: Some(fence) }
    }

    /// Release the fence explicitly. Dropping the guard releases it too.
    pub fn unlock(mut self) -> Result<(), PortError> {
        match self.fence.take() {
            Some(fence) => fence.unlock(),
            None => Ok(()),
        }
    }
}

impl Drop for ProfileWriteGuard {
    fn drop(&mut self) {
        drop(self.fence.take());
    }
}

/// Native fence: the process-local boundary plus the OS lock owned by the
/// worker thread.
struct NativeProfileWriteFence {
    _local: OwnedMutexGuard<()>,
    _cross_process: ThreadedInstanceLockGuard,
}

impl ProfileWriteFence for NativeProfileWriteFence {
    fn unlock(self: Box<Self>) -> Result<(), PortError> {
        drop(self);
        Ok(())
    }
}

/// Acquire the process-local and stable OS lock in the shared profile order
/// through the native platform.
pub async fn lock_profile_writes_for_dir(
    layout: &ProfileStoreLayout,
    config_dir: &Path,
) -> Result<ProfileWriteGuard, PortError> {
    let fence = NativeStorePlatform
        .lock_profile_writes(layout, config_dir)
        .await?;
    Ok(ProfileWriteGuard::new(fence))
}

/// Native implementation behind
/// [`crate::platform::StorePlatform::lock_profile_writes`].
pub(crate) async fn lock_profile_writes_native(
    layout: &ProfileStoreLayout,
    config_dir: &Path,
) -> Result<Box<dyn ProfileWriteFence>, PortError> {
    let boundary = shared_boundary(config_dir).map_err(map_io_error)?;
    let lock_path = profile_write_lock_path(layout, config_dir).await?;
    let cross_process = loop {
        let attempt_path = lock_path.clone();
        let attempt = spawn_blocking(move || try_acquire_threaded_instance_lock(&attempt_path))
            .await
            .map_err(|error| PortError::Failed(format!("profile write lock worker: {error}")))?
            .map_err(|error| PortError::Io(error.to_string()))?;
        if let Some(guard) = attempt {
            break guard;
        }
        sleep(Duration::from_millis(10)).await;
    };
    let local = boundary.lock_owned().await;
    Ok(Box::new(NativeProfileWriteFence {
        _local: local,
        _cross_process: cross_process,
    }))
}

fn shared_boundary(root: &Path) -> io::Result<Arc<sync::Mutex<()>>> {
    static STORES: OnceLock<Mutex<HashMap<PathBuf, Weak<sync::Mutex<()>>>>> = OnceLock::new();
    let root = canonical_root(root)?;
    let mut stores = STORES
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .map_err(|_| io::Error::other("profile boundary registry poisoned"))?;
    stores.retain(|_, boundary| boundary.strong_count() > 0);
    if let Some(boundary) = stores.get(&root).and_then(Weak::upgrade) {
        return Ok(boundary);
    }
    let boundary = Arc::new(sync::Mutex::new(()));
    stores.insert(root, Arc::downgrade(&boundary));
    Ok(boundary)
}

fn canonical_root(root: &Path) -> io::Result<PathBuf> {
    let absolute = if root.is_absolute() {
        root.to_path_buf()
    } else {
        current_dir()?.join(root)
    };
    let mut ancestor = absolute.as_path();
    let mut suffix = Vec::new();
    loop {
        match canonicalize(ancestor) {
            Ok(mut canonical) => {
                for component in suffix.into_iter().rev() {
                    canonical.push(component);
                }
                return Ok(canonical);
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {
                let Some(name) = ancestor.file_name() else {
                    return Err(error);
                };
                suffix.push(name.to_os_string());
                ancestor = ancestor.parent().ok_or(error)?;
            }
            Err(error) => return Err(error),
        }
    }
}

async fn profile_write_lock_path(
    layout: &ProfileStoreLayout,
    config_dir: &Path,
) -> Result<PathBuf, PortError> {
    let engine = config_dir
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .ok_or_else(|| PortError::Io("engine config directory has no name".into()))?;
    let parent = config_dir
        .parent()
        .ok_or_else(|| PortError::Io("engine config directory has no parent".into()))?;
    let lock_name = layout.profile_write_lock_name(engine);
    let parent = parent.to_path_buf();
    let canonical_parent = spawn_blocking(move || {
        NativeStorePlatform.create_dir_all(&parent)?;
        NativeStorePlatform.canonicalize(&parent)
    })
    .await
    .map_err(|error| PortError::Failed(format!("profile write lock path worker: {error}")))??;
    Ok(canonical_parent.join(lock_name))
}

#[cfg(test)]
#[path = "file_lock_tests.rs"]
mod tests;
