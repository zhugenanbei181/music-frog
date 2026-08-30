//! Profile/config management for the mihomo infiltrator.
//!
//! [`ConfigManager`] owns the config directory and the settings file
//! (`config.toml`). The implementation is split along business seams, with
//! public paths unchanged via re-exports:
//!
//! * `profiles` — profile CRUD and per-profile metadata
//! * `active` — active-profile selection and settings-file plumbing
//! * `defaults` — default-config bootstrap and proxy-port conflict repair
//! * `controller` — external-controller endpoint management
//! * `paths` — profile name validation and canonicalized path construction
//! * `subscription_store` — credential-store persistence of subscription URLs
//! * `metadata` — settings-TOML mapping helpers

mod active;
mod controller;
mod defaults;
#[cfg(test)]
mod manager_test;
mod metadata;
mod paths;
mod profiles;
mod subscription_store;

pub use paths::validate_profile_name;

use mihomo_api::error::Result;
use mihomo_platform::paths::get_home_dir;
use mihomo_platform::traits::{CredentialStore, DefaultCredentialStore};
use std::path::PathBuf;

pub struct ConfigManager<S: CredentialStore = DefaultCredentialStore> {
    config_dir: PathBuf,
    settings_file: PathBuf,
    credential_store: S,
}

impl<S: CredentialStore> ConfigManager<S> {
    pub fn new_with_store(credential_store: S) -> Result<Self> {
        let home = get_home_dir()?;
        Self::with_home_and_store(home, credential_store)
    }

    pub fn with_home_and_store(home: PathBuf, credential_store: S) -> Result<Self> {
        let config_dir = home.join("configs");
        let settings_file = home.join("config.toml");

        Ok(Self {
            config_dir,
            settings_file,
            credential_store,
        })
    }
}

impl ConfigManager<DefaultCredentialStore> {
    pub fn new() -> Result<Self> {
        Self::new_with_store(DefaultCredentialStore::default())
    }

    pub fn with_home(home: PathBuf) -> Result<Self> {
        Self::with_home_and_store(home, DefaultCredentialStore::default())
    }
}
