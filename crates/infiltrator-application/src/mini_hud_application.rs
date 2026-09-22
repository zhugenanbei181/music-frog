//! Mini HUD placement use-cases over the settings store and the host window
//! port.
//!
//! The geometry itself (clamp + edge snap) is the shared contract; this layer
//! owns the persistence round trip and the honest host outcome. A host with
//! no floating-window adapter keeps the placement stored and reports
//! [`MiniHudHostOutcome::Unsupported`] — it never claims the window moved.

use crate::settings_application::SettingsApplication;
use infiltrator_contract::error::Failure;
use infiltrator_contract::mini_hud::{
    MiniHudDisplay, MiniHudHostOutcome, MiniHudPlacement, MiniHudSnapPlacement,
};
use infiltrator_ports::mini_hud_window::MiniHudWindowPort;
use std::sync::Arc;

/// Edge-snap threshold in logical pixels: an 8px approach to a screen edge
/// latches the HUD flush to that edge.
pub const MINI_HUD_SNAP_THRESHOLD_PX: u32 = 8;

/// One placement write, with the snap that was applied and the honest host
/// outcome.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MiniHudPlacementReport {
    pub placement: MiniHudPlacement,
    pub snap: MiniHudSnapPlacement,
    pub host: MiniHudHostOutcome,
}

#[derive(Clone)]
pub struct MiniHudApplication {
    settings: SettingsApplication,
    window: Option<Arc<dyn MiniHudWindowPort>>,
}

impl MiniHudApplication {
    pub fn new(settings: SettingsApplication) -> Self {
        Self {
            settings,
            window: None,
        }
    }

    /// Inject the host floating-window adapter. Without one every apply call
    /// answers a typed unsupported.
    pub fn with_window_port(
        settings: SettingsApplication,
        window: Arc<dyn MiniHudWindowPort>,
    ) -> Self {
        Self {
            settings,
            window: Some(window),
        }
    }

    pub fn from_store(store: Arc<dyn infiltrator_ports::settings_store::SettingsStore>) -> Self {
        Self::new(SettingsApplication::new(store))
    }

    /// The persisted placement (defaults for a fresh settings file).
    pub async fn placement(&self) -> Result<MiniHudPlacement, Failure> {
        Ok(self.settings.load().await?.mini_hud)
    }

    /// Move the HUD to `(x, y)`: clamp into the display when the host has
    /// reported one, latch to the edges within the snap threshold, persist the
    /// result, then hand it to the host window port.
    pub async fn place(
        &self,
        x: i32,
        y: i32,
        display: Option<MiniHudDisplay>,
    ) -> Result<MiniHudPlacementReport, Failure> {
        let placement = self.settings.load().await?.mini_hud;
        self.place_from(placement, x, y, display).await
    }

    /// The same move, rebased on an already-loaded placement so a drag can
    /// update the setting without re-reading it between frames.
    pub async fn place_from(
        &self,
        current: MiniHudPlacement,
        x: i32,
        y: i32,
        display: Option<MiniHudDisplay>,
    ) -> Result<MiniHudPlacementReport, Failure> {
        let moved = MiniHudPlacement { x, y, ..current };
        let snap = match display {
            Some(display) => moved
                .clamped_to(display)
                .snapped_to_edges(display, MINI_HUD_SNAP_THRESHOLD_PX),
            // No display fact from the host: persist the raw coordinates
            // rather than inventing a screen rectangle to clamp against.
            None => MiniHudSnapPlacement {
                placement: moved,
                snapped_left: false,
                snapped_right: false,
                snapped_top: false,
                snapped_bottom: false,
            },
        };
        self.persist(snap.placement).await?;
        let host = self.apply_to_host(snap.placement).await;
        Ok(MiniHudPlacementReport {
            placement: snap.placement,
            snap,
            host,
        })
    }

    /// Persist the pin state (always-on-top) and apply it to the host window.
    pub async fn set_pinned(&self, pinned: bool) -> Result<MiniHudPlacementReport, Failure> {
        let placement = self.settings.load().await?.mini_hud;
        self.set_pinned_from(placement, pinned).await
    }

    /// Rebase the pin state on an already-loaded placement.
    pub async fn set_pinned_from(
        &self,
        current: MiniHudPlacement,
        pinned: bool,
    ) -> Result<MiniHudPlacementReport, Failure> {
        let placement = current.with_pinned(pinned);
        self.persist(placement).await?;
        let host = self.apply_to_host(placement).await;
        Ok(MiniHudPlacementReport {
            placement,
            snap: MiniHudSnapPlacement {
                placement,
                snapped_left: false,
                snapped_right: false,
                snapped_top: false,
                snapped_bottom: false,
            },
            host,
        })
    }

    /// Ask the host to show/hide the floating window. The visibility is not a
    /// persisted setting (the product default is hidden); only the placement
    /// and pin state persist.
    pub async fn set_visible(&self, visible: bool) -> MiniHudHostOutcome {
        match self.window.as_ref() {
            Some(window) => match window.set_visible(visible).await {
                Ok(outcome) => outcome,
                Err(error) => MiniHudHostOutcome::Unsupported {
                    reason: format!("HUD 窗口调用失败: {error:?}"),
                },
            },
            None => unsupported_reason(),
        }
    }

    async fn apply_to_host(&self, placement: MiniHudPlacement) -> MiniHudHostOutcome {
        match self.window.as_ref() {
            Some(window) => match window.apply_placement(placement).await {
                Ok(outcome) => outcome,
                Err(error) => MiniHudHostOutcome::Unsupported {
                    reason: format!("HUD 窗口调用失败: {error:?}"),
                },
            },
            None => unsupported_reason(),
        }
    }

    async fn persist(&self, placement: MiniHudPlacement) -> Result<(), Failure> {
        self.settings
            .update(move |settings| settings.mini_hud = placement)
            .await
    }
}

fn unsupported_reason() -> MiniHudHostOutcome {
    MiniHudHostOutcome::Unsupported {
        reason: "当前宿主未提供悬浮窗适配器，坐标已持久化但窗口未移动".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use infiltrator_domain::settings::AppSettings;
    use infiltrator_ports::error::PortError;
    use infiltrator_ports::settings_store::SettingsStore;
    use std::sync::Mutex;

    #[derive(Default)]
    struct MemorySettingsStore {
        settings: Mutex<AppSettings>,
    }

    #[async_trait]
    impl SettingsStore for MemorySettingsStore {
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

    #[derive(Default)]
    struct RecordingWindow {
        applied: Mutex<Vec<MiniHudPlacement>>,
        visible: Mutex<Vec<bool>>,
    }

    #[async_trait]
    impl MiniHudWindowPort for RecordingWindow {
        async fn apply_placement(
            &self,
            placement: MiniHudPlacement,
        ) -> Result<MiniHudHostOutcome, PortError> {
            self.applied.lock().expect("lock").push(placement);
            Ok(MiniHudHostOutcome::Applied)
        }

        async fn set_visible(&self, visible: bool) -> Result<MiniHudHostOutcome, PortError> {
            self.visible.lock().expect("lock").push(visible);
            Ok(MiniHudHostOutcome::Applied)
        }
    }

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("runtime")
    }

    fn display() -> MiniHudDisplay {
        MiniHudDisplay::new(0, 0, 1920, 1080)
    }

    #[test]
    fn place_snaps_clamps_persists_and_reports_the_host_outcome() {
        let store = Arc::new(MemorySettingsStore::default());
        let window = Arc::new(RecordingWindow::default());
        let application = MiniHudApplication::with_window_port(
            SettingsApplication::new(store.clone()),
            window.clone(),
        );
        let runtime = runtime();

        // Near the top-left corner: both edges latch and the window lands at 0,0.
        let report = runtime
            .block_on(application.place(4, 5, Some(display())))
            .expect("place");
        assert!(report.snap.snapped_left && report.snap.snapped_top);
        assert_eq!(report.placement, MiniHudPlacement::new(0, 0));
        assert_eq!(report.host, MiniHudHostOutcome::Applied);
        assert_eq!(
            runtime.block_on(application.placement()).expect("load"),
            MiniHudPlacement::new(0, 0),
            "the snapped placement is what persists"
        );
        assert_eq!(
            window.applied.lock().expect("lock").as_slice(),
            &[MiniHudPlacement::new(0, 0)]
        );

        // A wildly off-screen coordinate is clamped onto the display first.
        let report = runtime
            .block_on(application.place(-900, 9000, Some(display())))
            .expect("place");
        assert_eq!(report.placement.x, 0);
        assert_eq!(report.placement.y, 1080 - 90);
        assert!(report.snap.snapped_left && report.snap.snapped_bottom);
    }

    #[test]
    fn a_host_without_the_window_adapter_reports_typed_unsupported() {
        let store = Arc::new(MemorySettingsStore::default());
        let application = MiniHudApplication::new(SettingsApplication::new(store));
        let runtime = runtime();

        let report = runtime
            .block_on(application.place(320, 240, Some(display())))
            .expect("place");
        assert_eq!(report.placement, MiniHudPlacement::new(320, 240));
        assert!(matches!(
            report.host,
            MiniHudHostOutcome::Unsupported { .. }
        ));
        assert!(matches!(
            runtime.block_on(application.set_visible(true)),
            MiniHudHostOutcome::Unsupported { .. }
        ));
        assert_eq!(
            runtime.block_on(application.placement()).expect("load"),
            MiniHudPlacement::new(320, 240),
            "the placement still persists without a window adapter"
        );
    }

    #[test]
    fn pin_state_round_trips_and_reaches_the_window_port() {
        let store = Arc::new(MemorySettingsStore::default());
        let window = Arc::new(RecordingWindow::default());
        let application =
            MiniHudApplication::with_window_port(SettingsApplication::new(store), window.clone());
        let runtime = runtime();

        let report = runtime.block_on(application.set_pinned(true)).expect("pin");
        assert!(report.placement.pinned);
        assert_eq!(report.host, MiniHudHostOutcome::Applied);
        assert!(report.snap.placement.pinned);
        assert_eq!(window.applied.lock().expect("lock").len(), 1);
        assert!(
            runtime
                .block_on(application.placement())
                .expect("load")
                .pinned
        );
    }
}
