//! Host-owned settings and data-directory adapter functions.

use infiltrator_domain::settings::AppSettings;
use mihomo_platform::defaults::DefaultCredentialStore;
use std::path::PathBuf;

pub fn home_dir() -> anyhow::Result<PathBuf> {
    mihomo_platform::paths::get_home_dir().map_err(|error| anyhow::anyhow!(error.to_string()))
}

pub async fn load_settings() -> anyhow::Result<AppSettings> {
    let path = crate::settings_io::settings_path(&home_dir()?)?;
    crate::settings_io::load_settings(&path).await
}

pub async fn load_settings_hydrated() -> anyhow::Result<AppSettings> {
    let path = crate::settings_io::settings_path(&home_dir()?)?;
    crate::settings_io::load_settings_hydrated(&path).await
}

pub async fn save_settings(settings: &AppSettings) -> anyhow::Result<()> {
    let path = crate::settings_io::settings_path(&home_dir()?)?;
    crate::settings_io::save_settings(&path, settings).await
}

pub async fn save_webdav_password(password: &str) -> anyhow::Result<()> {
    crate::settings_io::save_webdav_password(
        &DefaultCredentialStore::default(),
        password,
    )
    .await
}

pub async fn clear_webdav_password() {
    crate::settings_io::clear_webdav_password(&DefaultCredentialStore::default()).await;
}
