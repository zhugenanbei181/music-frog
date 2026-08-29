#[cfg(target_os = "android")]
pub mod android;
pub mod android_bridge;
#[cfg(not(target_os = "android"))]
pub mod desktop;
pub mod instance_lock;
pub mod paths;
pub mod traits;

use crate::traits::DataDirProvider;

/// Shared test lock for cross-crate synchronization of global state (e.g. HOME_DIR_OVERRIDE).
/// Using tokio's Mutex to allow holding across .await points.
pub static TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

pub fn apply_data_dir_override<P: DataDirProvider>(provider: &P) {
    if let Some(path) = provider.data_dir() {
        paths::set_home_dir_override(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(not(target_os = "android"))]
    use crate::desktop::DesktopDataDirProvider;
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

    #[tokio::test]
    async fn test_apply_data_dir_override_none() {
        let _guard = TEST_LOCK.lock().await;
        struct NoneProvider;
        impl DataDirProvider for NoneProvider {
            fn data_dir(&self) -> Option<PathBuf> {
                None
            }
        }
        let provider = NoneProvider;
        paths::clear_home_dir_override();
        apply_data_dir_override(&provider);
    }

    #[tokio::test]
    async fn test_apply_default_data_dir_override() {
        let _guard = TEST_LOCK.lock().await;
        let provider = DesktopDataDirProvider;
        apply_data_dir_override(&provider);
    }
}
