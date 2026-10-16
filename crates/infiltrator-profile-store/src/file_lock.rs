//! Shared in-process and cross-process profile write fences.

use infiltrator_platform::instance_lock::{
    ThreadedInstanceLockGuard, try_acquire_threaded_instance_lock,
};
use infiltrator_ports::error::PortError;
use std::collections::HashMap;
use std::env::current_dir;
use std::fs::canonicalize;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::Duration;
use tokio::fs;
use tokio::sync::{self, OwnedMutexGuard};
use tokio::task::spawn_blocking;
use tokio::time::sleep;

/// A held lock that fences profile reads and writes for one engine namespace.
pub struct ProfileWriteGuard {
    _local: OwnedMutexGuard<()>,
    _cross_process: ThreadedInstanceLockGuard,
}

/// Acquire the process-local and stable OS lock in the shared profile order.
pub async fn lock_profile_writes_for_dir(
    config_dir: &Path,
) -> Result<ProfileWriteGuard, PortError> {
    let boundary = shared_boundary(config_dir).map_err(io_error)?;
    let lock_path = profile_write_lock_path(config_dir).await?;
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
    Ok(ProfileWriteGuard {
        _local: local,
        _cross_process: cross_process,
    })
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

async fn profile_write_lock_path(config_dir: &Path) -> Result<PathBuf, PortError> {
    let engine = config_dir
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .ok_or_else(|| PortError::Io("engine config directory has no name".into()))?;
    let parent = config_dir
        .parent()
        .ok_or_else(|| PortError::Io("engine config directory has no parent".into()))?;
    fs::create_dir_all(parent).await.map_err(io_error)?;
    let canonical_parent = fs::canonicalize(parent).await.map_err(io_error)?;
    Ok(canonical_parent.join(format!(".profile-write-{engine}.lock")))
}

fn io_error(error: io::Error) -> PortError {
    match error.kind() {
        ErrorKind::PermissionDenied => PortError::PermissionDenied(error.to_string()),
        _ => PortError::Io(error.to_string()),
    }
}

#[cfg(test)]
#[path = "file_lock_tests.rs"]
mod tests;
