mod android_bridge;
mod paths;
pub mod traits;

pub use android_bridge::{
    clear_android_bridge, get_android_bridge, set_android_bridge, AndroidBridge,
};
pub use paths::{get_home_dir, set_home_dir_override, clear_home_dir_override};
pub use traits::{
    CoreController, CredentialStore, DataDirProvider, DefaultCredentialStore,
    DefaultDataDirProvider,
};

#[cfg(not(target_os = "android"))]
pub mod desktop;
#[cfg(not(target_os = "android"))]
pub use desktop::{DesktopDataDirProvider, KeyringCredentialStore, ProcessCoreController};

#[cfg(target_os = "android")]
pub mod android;
#[cfg(target_os = "android")]
pub use android::{AndroidCoreController, AndroidCredentialStore, AndroidDataDirProvider};

pub fn apply_data_dir_override<P: DataDirProvider>(provider: &P) {
    if let Some(path) = provider.data_dir() {
        let _ = set_home_dir_override(path);
    }
}

pub fn apply_default_data_dir_override() {
    let provider = DefaultDataDirProvider::default();
    apply_data_dir_override(&provider);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_apply_data_dir_override() {
        struct TestProvider {
            path: Option<PathBuf>,
        }

        impl DataDirProvider for TestProvider {
            fn data_dir(&self) -> Option<PathBuf> {
                self.path.clone()
            }
        }

        // Clear any existing override
        paths::clear_home_dir_override();

        let provider = TestProvider {
            path: Some(PathBuf::from("/test/path")),
        };
        apply_data_dir_override(&provider);

        let result = get_home_dir().unwrap();
        assert_eq!(result, PathBuf::from("/test/path"));
    }

    #[test]
    fn test_apply_data_dir_override_none() {
        struct TestProvider;

        impl DataDirProvider for TestProvider {
            fn data_dir(&self) -> Option<PathBuf> {
                None
            }
        }

        // Clear any existing override
        paths::clear_home_dir_override();

        let provider = TestProvider;
        apply_data_dir_override(&provider);

        // Should not set override if we can check it, but get_home_dir might return default
    }

    #[test]
    fn test_apply_default_data_dir_override() {
        // This test just verifies the function runs without panicking
        // The actual behavior depends on the test environment
        apply_default_data_dir_override();
    }
}
