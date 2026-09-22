//! Iced-side access to the shared Mini HUD placement facade.
//!
//! Geometry and the persistence rule live in
//! `infiltrator_contract::mini_hud` / `infiltrator_application::mini_hud_application`;
//! this module only hands the facade the host settings store and reports the
//! stored placement back to the Elm state.

use infiltrator_application::mini_hud_application::MiniHudApplication;
use infiltrator_application::settings_application::SettingsApplication;
use infiltrator_contract::error::InfiltratorError;
use infiltrator_contract::mini_hud::{MiniHudDisplay, MiniHudPlacement};

async fn application() -> Result<MiniHudApplication, InfiltratorError> {
    let store = crate::host::storage::settings_store()
        .await
        .map_err(|error| InfiltratorError::Config(error.to_string()))?;
    Ok(MiniHudApplication::new(SettingsApplication::new(store)))
}

pub async fn placement() -> Result<MiniHudPlacement, InfiltratorError> {
    application()
        .await?
        .placement()
        .await
        .map_err(|failure| InfiltratorError::Config(failure.message))
}

/// Persist one drag/nudge: the shared application clamps against the display
/// (when the host reported one), latches to the screen edges and stores the
/// result. A host without a floating-window adapter still persists the
/// placement and reports the typed unsupported outcome.
pub async fn place(
    current: MiniHudPlacement,
    x: i32,
    y: i32,
    display: Option<MiniHudDisplay>,
) -> Result<MiniHudPlacement, InfiltratorError> {
    application()
        .await?
        .place_from(current, x, y, display)
        .await
        .map(|report| report.placement)
        .map_err(|failure| InfiltratorError::Config(failure.message))
}

pub async fn set_pinned(
    current: MiniHudPlacement,
    pinned: bool,
) -> Result<MiniHudPlacement, InfiltratorError> {
    application()
        .await?
        .set_pinned_from(current, pinned)
        .await
        .map(|report| report.placement)
        .map_err(|failure| InfiltratorError::Config(failure.message))
}
