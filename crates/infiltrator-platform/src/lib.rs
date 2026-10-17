use tokio::sync::Mutex;
#[cfg(target_os = "android")]
pub mod android;
pub mod android_bridge;
pub mod crash_reporter;
pub mod credentials;
pub mod defaults;
#[cfg(not(target_os = "android"))]
pub mod desktop;
pub mod durable_replace;
pub mod instance_ipc;
pub mod instance_lock;
pub mod interface_watcher;
pub mod package_verifier;
pub mod paths;
pub mod power;
pub mod sandbox_validator;

use infiltrator_ports::data_dir::DataDirProvider;

/// Shared test lock for cross-crate synchronization of global state (e.g. HOME_DIR_OVERRIDE).
/// Using tokio's Mutex to allow holding across .await points.
pub static TEST_LOCK: Mutex<()> = Mutex::const_new(());

pub fn apply_data_dir_override<P: DataDirProvider>(provider: &P) {
    if let Some(path) = provider.data_dir() {
        paths::set_home_dir_override(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    struct MockProvider {
        path: PathBuf,
    }

    impl DataDirProvider for MockProvider {
        fn data_dir(&self) -> Option<PathBuf> {
            Some(self.path.clone())
        }
    }

    #[tokio::test]
    async fn test_apply_data_dir_override() {
        let _guard = TEST_LOCK.lock().await;
        let path = PathBuf::from("/test/path");
        let provider = MockProvider { path: path.clone() };
        apply_data_dir_override(&provider);
        assert_eq!(paths::get_home_dir().unwrap(), path);
        paths::clear_home_dir_override();
    }
}
