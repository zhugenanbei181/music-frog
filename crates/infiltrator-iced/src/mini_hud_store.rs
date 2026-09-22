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
mod tests {
    use super::*;
    use async_trait::async_trait;
    use infiltrator_domain::settings::AppSettings;
    use infiltrator_ports::error::PortError;
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct MemorySettingsStore {
        settings: Mutex<AppSettings>,
    }

    #[async_trait]
    impl infiltrator_ports::settings_store::SettingsStore for MemorySettingsStore {
        async fn load(&self) -> Result<AppSettings, PortError> {
            Ok(self.settings.lock().expect("lock").clone())
        }

        async fn load_hydrated(&self) -> Result<AppSettings, PortError> {
            self.load().await
        }

        async fn save(&self, settings: &AppSettings) -> Result<(), PortError> {
            *self.settings.lock().expect("lock") = settings.clone();
            Ok(())
        }
    }

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("runtime")
    }

    /// The Iced composition path: the shared application hands the placement
    /// to the desktop host adapter, which reaches this surface's live window
    /// handle; the handle queues it for the update-path window task.
    #[test]
    fn a_persisted_placement_reaches_the_iced_window_handle() {
        let handle = crate::mini_hud_window::install_host_handle();
        handle.mark_live(false);
        let _ = handle.take_pending();
        let application = facade(Arc::new(MemorySettingsStore::default()));
        let display = infiltrator_contract::mini_hud::MiniHudDisplay::new(0, 0, 1920, 1080);
        let runtime = runtime();

        // The window id is not resolved yet: the desktop adapter stays honest.
        let report = runtime
            .block_on(application.place_from(MiniHudPlacement::default(), 8, 12, Some(display)))
            .expect("place");
        assert!(matches!(
            report.host,
            infiltrator_contract::mini_hud::MiniHudHostOutcome::Unsupported { .. }
        ));
        assert!(handle.take_pending().is_empty());

        // With the window live the same call latches to the corner and queues
        // the exact placement for the window task.
        handle.mark_live(true);
        let report = runtime
            .block_on(application.place_from(MiniHudPlacement::default(), 8, 12, Some(display)))
            .expect("place");
        assert_eq!(
            report.host,
            infiltrator_contract::mini_hud::MiniHudHostOutcome::Applied
        );
        assert_eq!(
            report.placement,
            MiniHudPlacement::new(0, 12),
            "the persisted placement is the edge-snapped one (left edge latched, y kept)"
        );
        assert_eq!(handle.take_pending(), vec![MiniHudPlacement::new(0, 12)]);
    }
}
