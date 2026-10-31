//! Host-owned settings and data-directory adapter functions.

use crate::error::CoreError;
use crate::settings_io;
use crate::settings_io::settings_path;
use infiltrator_domain::settings::AppSettings;
use infiltrator_platform::defaults::DefaultCredentialStore;
use infiltrator_platform::paths::get_home_dir;
use std::path::PathBuf;

pub fn home_dir() -> Result<PathBuf, CoreError> {
    get_home_dir().map_err(|error| CoreError::other(error.to_string()))
}

pub async fn load_settings() -> Result<AppSettings, CoreError> {
    let path = settings_path(&home_dir()?)?;
    settings_io::load_settings(&path).await
}

pub async fn load_settings_hydrated() -> Result<AppSettings, CoreError> {
    let path = settings_path(&home_dir()?)?;
    settings_io::load_settings_hydrated(&path).await
}

pub async fn save_settings(settings: &AppSettings) -> Result<(), CoreError> {
    let path = settings_path(&home_dir()?)?;
    settings_io::save_settings(&path, settings).await
}

pub async fn save_webdav_password(password: &str) -> Result<(), CoreError> {
    settings_io::save_webdav_password(&DefaultCredentialStore::default(), password).await
}

pub async fn clear_webdav_password() {
    settings_io::clear_webdav_password(&DefaultCredentialStore::default()).await;
}
