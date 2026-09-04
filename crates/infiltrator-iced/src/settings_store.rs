//! Iced-side access to the application settings facade.

use infiltrator_application::settings_application::SettingsApplication;
use infiltrator_contract::error::InfiltratorError;
use infiltrator_domain::settings::AppSettings;

async fn application() -> Result<SettingsApplication, InfiltratorError> {
    let store = infiltrator_desktop::storage::settings_store()
        .await
        .map_err(|error| InfiltratorError::Config(error.to_string()))?;
    Ok(SettingsApplication::new(store))
}

pub async fn load() -> Result<AppSettings, InfiltratorError> {
    application()
        .await?
        .load()
        .await
        .map_err(|failure| InfiltratorError::Config(failure.message))
}

pub async fn load_hydrated() -> Result<AppSettings, InfiltratorError> {
    application()
        .await?
        .load_hydrated()
        .await
        .map_err(|failure| InfiltratorError::Config(failure.message))
}

pub async fn save(settings: &AppSettings) -> Result<(), InfiltratorError> {
    application()
        .await?
        .save(settings)
        .await
        .map_err(|failure| InfiltratorError::Config(failure.message))
}

pub async fn update<F>(update: F) -> Result<(), InfiltratorError>
where
    F: FnOnce(&mut AppSettings),
{
    application()
        .await?
        .update(update)
        .await
        .map_err(|failure| InfiltratorError::Config(failure.message))
}
