//! Iced-side access to the shared shortcut application facade.
//!
//! The registry grammar lives in `infiltrator_contract::shortcuts`; the
//! persistence round trip and conflict rejection live in
//! `infiltrator_application::shortcut_application`; this module is only the
//! desktop-composition wiring that hands the facade the host settings store.

use infiltrator_application::settings_application::SettingsApplication;
use infiltrator_application::shortcut_application::ShortcutApplication;
use infiltrator_contract::error::InfiltratorError;
use infiltrator_contract::shortcuts::{ShortcutAction, ShortcutChord, ShortcutRegistry};

async fn application() -> Result<ShortcutApplication, InfiltratorError> {
    let store = crate::host::storage::settings_store()
        .await
        .map_err(|error| InfiltratorError::Config(error.to_string()))?;
    Ok(ShortcutApplication::new(SettingsApplication::new(store)))
}

pub async fn load() -> Result<ShortcutRegistry, InfiltratorError> {
    application()
        .await?
        .registry()
        .await
        .map_err(|failure| InfiltratorError::Config(failure.message))
}

pub async fn capture(
    action: ShortcutAction,
    chord: ShortcutChord,
) -> Result<ShortcutRegistry, InfiltratorError> {
    application()
        .await?
        .capture(action, chord)
        .await
        .map_err(|failure| InfiltratorError::Config(failure.message))
}

pub async fn set_enabled(
    action: ShortcutAction,
    enabled: bool,
) -> Result<ShortcutRegistry, InfiltratorError> {
    application()
        .await?
        .set_enabled(action, enabled)
        .await
        .map_err(|failure| InfiltratorError::Config(failure.message))
}

pub async fn reset_action(action: ShortcutAction) -> Result<ShortcutRegistry, InfiltratorError> {
    application()
        .await?
        .reset_action(action)
        .await
        .map_err(|failure| InfiltratorError::Config(failure.message))
}
