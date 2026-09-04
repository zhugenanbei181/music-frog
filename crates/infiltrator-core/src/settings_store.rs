//! Filesystem/keyring adapter for the runtime-neutral settings port.

use infiltrator_domain::settings::AppSettings;
use infiltrator_ports::error::PortError;
use infiltrator_ports::secure_store::SecureStore;
use infiltrator_ports::settings_store::SettingsStore;
use mihomo_platform::defaults::DefaultCredentialStore;
use std::path::PathBuf;

pub struct FileSettingsStore<S = DefaultCredentialStore> {
    home: PathBuf,
    secure_store: S,
}

impl<S> FileSettingsStore<S> {
    pub fn new(home: PathBuf, secure_store: S) -> Self {
        Self { home, secure_store }
    }
}

impl FileSettingsStore<DefaultCredentialStore> {
    pub fn for_home(home: PathBuf) -> Self {
        Self::new(home, DefaultCredentialStore::default())
    }
}

#[async_trait::async_trait]
impl<S> SettingsStore for FileSettingsStore<S>
where
    S: SecureStore,
{
    async fn load(&self) -> Result<AppSettings, PortError> {
        let path = crate::settings_io::settings_path(&self.home)
            .map_err(|error| PortError::Io(error.to_string()))?;
        crate::settings_io::load_settings(&path)
            .await
            .map_err(|error| PortError::Io(error.to_string()))
    }

    async fn load_hydrated(&self) -> Result<AppSettings, PortError> {
        let path = crate::settings_io::settings_path(&self.home)
            .map_err(|error| PortError::Io(error.to_string()))?;
        crate::settings_io::load_settings_hydrated_with_store(&path, &self.secure_store)
            .await
            .map_err(|error| PortError::Io(error.to_string()))
    }

    async fn save(&self, settings: &AppSettings) -> Result<(), PortError> {
        let path = crate::settings_io::settings_path(&self.home)
            .map_err(|error| PortError::Io(error.to_string()))?;
        crate::settings_io::save_settings(&path, settings)
            .await
            .map_err(|error| PortError::Io(error.to_string()))
    }
}

pub fn for_current_home() -> anyhow::Result<FileSettingsStore> {
    let home = mihomo_platform::paths::get_home_dir()?;
    Ok(FileSettingsStore::for_home(home))
}
