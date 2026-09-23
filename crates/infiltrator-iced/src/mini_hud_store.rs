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

/// The shared application facade over the desktop host's floating-window port.
/// The port is bound to the Iced window handle by
/// [`crate::mini_hud_window::install_host_handle`]; an unbound host keeps the
/// typed unsupported outcome.
fn facade(
    store: std::sync::Arc<dyn infiltrator_ports::settings_store::SettingsStore>,
) -> MiniHudApplication {
    MiniHudApplication::with_window_port(
        SettingsApplication::new(store),
        crate::host::mini_hud::window_port(),
    )
}

async fn application() -> Result<MiniHudApplication, InfiltratorError> {
    let store = crate::host::storage::settings_store()
        .await
        .map_err(|error| InfiltratorError::Config(error.to_string()))?;
    Ok(facade(store))
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

#[cfg(test)]
#[path = "../tests/gui/mini_hud_store_tests.rs"]
mod mini_hud_store_tests;
