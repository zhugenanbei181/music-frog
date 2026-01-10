mod paths;
pub mod traits;

pub use paths::{get_home_dir, set_home_dir_override};
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
