//! Service-process-independent Android native initialization.
//!
//! BANDROID-001/002: the `:vpn` process is created by the system without any
//! Activity, so it must not rely on the UI process having run first. This
//! module owns the typed, process-local initialization seam shared by the JNI
//! `nativeInit` entry and a direct service-side caller: it validates the
//! sandbox directories, installs the rustls crypto provider and points the
//! mihomo home at the application data directory.
//!
//! The returned handle records only what *this* process initialized. It is
//! deliberately not a cross-process protocol: durable VPN/kernel/config facts
//! remain owned by the service and application layers, and the process-local
//! home override is never treated as shared state.

use crate::tls::ensure_rustls_provider;
use mihomo_platform::paths::set_home_dir_override;
use std::path::PathBuf;

/// Directories the Android host hands to the native process.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AndroidProcessDirs {
    data_dir: PathBuf,
    cache_dir: PathBuf,
}

impl AndroidProcessDirs {
    pub fn data_dir(&self) -> &PathBuf {
        &self.data_dir
    }

    pub fn cache_dir(&self) -> &PathBuf {
        &self.cache_dir
    }
}

/// Typed outcome of one process-local initialization.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AndroidProcessInit {
    dirs: AndroidProcessDirs,
}

impl AndroidProcessInit {
    pub fn dirs(&self) -> &AndroidProcessDirs {
        &self.dirs
    }

    pub fn data_dir(&self) -> &PathBuf {
        self.dirs.data_dir()
    }

    pub fn cache_dir(&self) -> &PathBuf {
        self.dirs.cache_dir()
    }
}

/// Typed rejection of an invalid initialization request.
#[derive(Clone, Debug, thiserror::Error, PartialEq, Eq)]
pub enum AndroidProcessInitError {
    #[error("android process {0} directory is empty")]
    EmptyDirectory(&'static str),
}

/// Initialize the native process from explicit sandbox directories.
///
/// Callable from the `:vpn` service process with no Activity present. The home
/// override is process-local state; a repeated call simply re-points it at the
/// same application data directory.
pub fn init_android_process(
    data_dir: impl Into<PathBuf>,
    cache_dir: impl Into<PathBuf>,
) -> Result<AndroidProcessInit, AndroidProcessInitError> {
    let dirs = AndroidProcessDirs {
        data_dir: data_dir.into(),
        cache_dir: cache_dir.into(),
    };
    if dirs.data_dir.as_os_str().is_empty() {
        return Err(AndroidProcessInitError::EmptyDirectory("data"));
    }
    if dirs.cache_dir.as_os_str().is_empty() {
        return Err(AndroidProcessInitError::EmptyDirectory("cache"));
    }
    ensure_rustls_provider();
    let _ = set_home_dir_override(dirs.data_dir.clone());
    Ok(AndroidProcessInit { dirs })
}

#[cfg(test)]
mod tests {
    use super::*;
    use mihomo_platform::TEST_LOCK;
    use mihomo_platform::paths::{clear_home_dir_override, get_home_dir};
    use std::env::temp_dir;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_dir(tag: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        temp_dir().join(format!("infiltrator-android-init-{tag}-{nanos}"))
    }

    #[test]
    fn empty_directories_are_rejected() {
        let cache = unique_dir("cache");
        assert_eq!(
            init_android_process("", cache).expect_err("empty data dir"),
            AndroidProcessInitError::EmptyDirectory("data")
        );
        let data = unique_dir("data");
        assert_eq!(
            init_android_process(data, "").expect_err("empty cache dir"),
            AndroidProcessInitError::EmptyDirectory("cache")
        );
    }

    #[tokio::test]
    async fn init_is_repeatable_and_records_the_directories() {
        let _guard = TEST_LOCK.lock().await;
        clear_home_dir_override();
        let data = unique_dir("data");
        let cache = unique_dir("cache");

        let first = init_android_process(data.clone(), cache.clone()).expect("first init");
        assert_eq!(first.data_dir(), &data);
        assert_eq!(first.cache_dir(), &cache);
        assert_eq!(first.dirs().data_dir(), &data);

        let second = init_android_process(data.clone(), cache.clone()).expect("second init");
        assert_eq!(second.data_dir(), &data);
        assert_eq!(get_home_dir().expect("home"), data);

        clear_home_dir_override();
    }
}
