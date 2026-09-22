//! Desktop Mini HUD window adapter (DUAL-15-04).
//!
//! The desktop host runtime owns the [`MiniHudWindowPort`]; the OS window it
//! drives is owned by the active surface, which registers a
//! [`MiniHudWindowHandle`] once its window id is resolved. Until a handle is
//! registered — and on hosts whose HUD is an in-window overlay (Bevy) or
//! headless — every call answers a typed
//! [`MiniHudHostOutcome::Unsupported`] instead of pretending the window moved.
//!
//! The shared singleton follows the desktop host idiom already used by
//! [`crate::pac_service::DesktopPacServicePort::shared`] so the composition
//! root and the surface adapter observe the same registration.

use async_trait::async_trait;
use infiltrator_contract::mini_hud::{MiniHudHostOutcome, MiniHudPlacement};
use infiltrator_ports::error::PortError;
use infiltrator_ports::mini_hud_window::{MiniHudWindowHandle, MiniHudWindowPort};
use std::sync::{Arc, Mutex, OnceLock};

type HandleSlot = Arc<Mutex<Option<Arc<dyn MiniHudWindowHandle>>>>;

/// Desktop adapter projecting the shared placement onto the surface-owned
/// floating HUD window.
#[derive(Clone)]
pub struct DesktopMiniHudWindow {
    handle: HandleSlot,
}

static SHARED_HANDLE: OnceLock<HandleSlot> = OnceLock::new();

impl DesktopMiniHudWindow {
    /// The process-wide adapter. Every composition root observes the same
    /// registered window handle.
    pub fn shared() -> Self {
        Self {
            handle: Arc::clone(SHARED_HANDLE.get_or_init(|| Arc::new(Mutex::new(None)))),
        }
    }

    /// Register the surface-owned window handle. Called when the surface has a
    /// live window (its window id resolved).
    pub fn bind(&self, handle: Arc<dyn MiniHudWindowHandle>) {
        if let Ok(mut slot) = self.handle.lock() {
            *slot = Some(handle);
        }
    }

    /// Forget the registered handle (window closed / detached).
    pub fn unbind(&self) {
        if let Ok(mut slot) = self.handle.lock() {
            *slot = None;
        }
    }

    fn bound_handle(&self) -> Option<Arc<dyn MiniHudWindowHandle>> {
        self.handle
            .lock()
            .ok()
            .and_then(|slot| slot.as_ref().map(Arc::clone))
    }
}

#[async_trait]
impl MiniHudWindowPort for DesktopMiniHudWindow {
    async fn apply_placement(
        &self,
        placement: MiniHudPlacement,
    ) -> Result<MiniHudHostOutcome, PortError> {
        match self.bound_handle() {
            Some(handle) if handle.apply_placement(placement) => Ok(MiniHudHostOutcome::Applied),
            Some(_) => Ok(MiniHudHostOutcome::Unsupported {
                reason: "悬浮窗句柄尚未就绪，坐标已持久化但窗口未移动".to_owned(),
            }),
            None => Ok(MiniHudHostOutcome::Unsupported {
                reason: "当前宿主未注册悬浮窗句柄，坐标已持久化但窗口未移动".to_owned(),
            }),
        }
    }

    async fn set_visible(&self, visible: bool) -> Result<MiniHudHostOutcome, PortError> {
        match self.bound_handle() {
            Some(handle) if handle.set_visible(visible) => Ok(MiniHudHostOutcome::Applied),
            Some(_) => Ok(MiniHudHostOutcome::Unsupported {
                reason: "当前宿主没有独立悬浮窗显隐（HUD 模式由外壳切换）".to_owned(),
            }),
            None => Ok(MiniHudHostOutcome::Unsupported {
                reason: "当前宿主未注册悬浮窗句柄，无法显示或隐藏 HUD".to_owned(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[derive(Default)]
    struct RecordingHandle {
        applied: Mutex<Vec<MiniHudPlacement>>,
        visible: Mutex<Vec<bool>>,
        live: AtomicBool,
    }

    impl RecordingHandle {
        fn live() -> Arc<Self> {
            let handle = Arc::new(Self::default());
            handle.live.store(true, Ordering::SeqCst);
            handle
        }
    }

    impl MiniHudWindowHandle for RecordingHandle {
        fn apply_placement(&self, placement: MiniHudPlacement) -> bool {
            if !self.live.load(Ordering::SeqCst) {
                return false;
            }
            self.applied.lock().expect("lock").push(placement);
            true
        }

        fn set_visible(&self, visible: bool) -> bool {
            if !self.live.load(Ordering::SeqCst) {
                return false;
            }
            self.visible.lock().expect("lock").push(visible);
            true
        }
    }

    fn detached() -> DesktopMiniHudWindow {
        DesktopMiniHudWindow {
            handle: Arc::new(Mutex::new(None)),
        }
    }

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("runtime")
    }

    #[test]
    fn a_bound_handle_receives_the_placement_and_the_visibility_request() {
        let adapter = detached();
        let handle = RecordingHandle::live();
        adapter.bind(handle.clone());
        let runtime = runtime();

        let placement = MiniHudPlacement::new(420, 260).with_pinned(true);
        assert_eq!(
            runtime
                .block_on(adapter.apply_placement(placement))
                .expect("apply"),
            MiniHudHostOutcome::Applied
        );
        assert_eq!(
            handle.applied.lock().expect("lock").as_slice(),
            &[placement],
            "the real window handle got the exact persisted placement"
        );
        assert_eq!(
            runtime
                .block_on(adapter.set_visible(true))
                .expect("visible"),
            MiniHudHostOutcome::Applied
        );
        assert_eq!(handle.visible.lock().expect("lock").as_slice(), &[true]);
    }

    #[test]
    fn an_unbound_or_stale_handle_reports_typed_unsupported() {
        let adapter = detached();
        let runtime = runtime();

        // No handle registered: the desktop host has no floating window yet.
        let outcome = runtime
            .block_on(adapter.apply_placement(MiniHudPlacement::new(10, 10)))
            .expect("apply");
        assert!(matches!(outcome, MiniHudHostOutcome::Unsupported { .. }));
        assert!(matches!(
            runtime
                .block_on(adapter.set_visible(false))
                .expect("visible"),
            MiniHudHostOutcome::Unsupported { .. }
        ));

        // A registered handle whose window is not live is the same honest
        // answer: nothing moved.
        let stale = Arc::new(RecordingHandle::default());
        adapter.bind(stale.clone());
        let outcome = runtime
            .block_on(adapter.apply_placement(MiniHudPlacement::new(10, 10)))
            .expect("apply");
        assert!(matches!(outcome, MiniHudHostOutcome::Unsupported { .. }));
        assert!(stale.applied.lock().expect("lock").is_empty());

        // Unbind restores the typed unsupported state after a live window.
        adapter.bind(RecordingHandle::live());
        adapter.unbind();
        let outcome = runtime
            .block_on(adapter.apply_placement(MiniHudPlacement::new(30, 30)))
            .expect("apply");
        assert!(matches!(outcome, MiniHudHostOutcome::Unsupported { .. }));
    }

    #[test]
    fn the_shared_adapter_is_one_registration_slot() {
        let first = DesktopMiniHudWindow::shared();
        let second = DesktopMiniHudWindow::shared();
        first.unbind();
        let handle = RecordingHandle::live();
        first.bind(handle.clone());
        // The second composition-root handle observes the same registration.
        let placement = MiniHudPlacement::new(64, 48);
        assert_eq!(
            runtime()
                .block_on(second.apply_placement(placement))
                .expect("apply"),
            MiniHudHostOutcome::Applied
        );
        assert_eq!(
            handle.applied.lock().expect("lock").as_slice(),
            &[placement]
        );
        second.unbind();
        assert!(first.bound_handle().is_none());
    }
}
