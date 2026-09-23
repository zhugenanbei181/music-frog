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
