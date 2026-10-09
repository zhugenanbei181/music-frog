//! BANDROID-005..008: the native Android host-fact adapter.
//!
//! The Kotlin Activity owns the real lifecycle, `WindowInsets`, IME and
//! `ClipboardManager` wiring. This module is the process-local, toolkit-neutral
//! seam between that host and the shared shell: it stores the latest pushed
//! facts, projects them onto the existing contract/ports vocabulary
//! ([`RenderCadence`], [`SafeAreaInsets`], [`SubscriptionImportPort`],
//! [`ImeCursorSupport`]) and exposes typed adapters. No JNI, `Context`, `View`
//! or Binder type crosses this boundary.
//!
//! The facts are pushed from the native host; they are not a cross-process
//! protocol. Durable VPN/kernel/config state stays with the service and
//! application owners, and the `:vpn` process is unaffected by UI lifecycle.

pub mod clipboard;
pub mod ime;
pub mod insets;
pub mod lifecycle;

use async_trait::async_trait;
use insets::NativeInsets;
use insets::RawInsetsPx;
use lifecycle::HostLifecycleFact;
use lifecycle::HostLifecycleState;
use lifecycle::HostLifecycleTracker;
use lifecycle::LifecycleUpdate;
use std::sync::Mutex;
use std::sync::OnceLock;

use infiltrator_ports::error::PortError;
use infiltrator_ports::touch_gesture::TouchGestureHostReport;
use infiltrator_ports::touch_gesture::TouchGesturePort;

/// The typed host facts the shared shell consumes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeHostFacts {
    pub lifecycle: HostLifecycleState,
    pub insets: NativeInsets,
}

impl Default for NativeHostFacts {
    fn default() -> Self {
        Self {
            lifecycle: HostLifecycleState::COLD,
            insets: NativeInsets::ZERO,
        }
    }
}

struct HostFactState {
    lifecycle: HostLifecycleTracker,
    insets: NativeInsets,
    touch_declared: bool,
}

impl HostFactState {
    fn new() -> Self {
        Self {
            lifecycle: HostLifecycleTracker::new(),
            insets: NativeInsets::ZERO,
            touch_declared: false,
        }
    }

    fn snapshot(&self) -> NativeHostFacts {
        NativeHostFacts {
            lifecycle: self.lifecycle.current(),
            insets: self.insets,
        }
    }
}

fn host_fact_state() -> &'static Mutex<HostFactState> {
    static STATE: OnceLock<Mutex<HostFactState>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(HostFactState::new()))
}

fn with_state<T>(f: impl FnOnce(&mut HostFactState) -> T) -> T {
    let mut guard = host_fact_state()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    f(&mut guard)
}

/// Push one lifecycle fact from the native Activity. Returns whether it was
/// applied or rejected as stale (a retired Activity generation).
pub fn push_lifecycle_fact(fact: HostLifecycleFact) -> LifecycleUpdate {
    with_state(|state| state.lifecycle.apply(fact))
}

/// Push one raw `WindowInsets` observation. Returns the typed conversion.
pub fn push_raw_insets(raw: RawInsetsPx) -> NativeInsets {
    let insets = NativeInsets::from_raw(raw);
    with_state(|state| {
        state.insets = insets;
        insets
    })
}

/// Declare that this host delivers touch (the native Activity adapter
/// registered). Until then the touch port stays typed unsupported.
pub fn declare_touch_host() {
    with_state(|state| state.touch_declared = true);
}

/// The latest typed facts (cold defaults before the host pushes any).
pub fn native_host_facts() -> NativeHostFacts {
    with_state(|state| state.snapshot())
}

/// Reset the process-local facts (Activity destroyed / tests).
pub fn clear_native_host_facts() {
    with_state(|state| {
        state.lifecycle = HostLifecycleTracker::new();
        state.insets = NativeInsets::ZERO;
        state.touch_declared = false;
    });
}

/// The Android host's typed touch and safe-area declaration.
///
/// Before the native adapter declares a touch host the report is the shared
/// typed-unsupported default; afterwards it carries the live safe-area insets.
/// The permanent safe area never includes the keyboard insets.
#[derive(Clone, Copy, Debug, Default)]
pub struct AndroidTouchGesturePort;

#[async_trait]
impl TouchGesturePort for AndroidTouchGesturePort {
    async fn touch_gesture_report(&self) -> Result<TouchGestureHostReport, PortError> {
        let (declared, insets) = with_state(|state| (state.touch_declared, state.insets));
        if !declared {
            return Ok(TouchGestureHostReport::default());
        }
        // Android delivers multi-touch; the host declares it here. The real
        // device regression remains a device-evidence item.
        Ok(TouchGestureHostReport::hosted(true, insets.safe_area))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::FutureExt;
    use insets::EdgeInsetsPx;
    use lifecycle::ActivityPhase;
    use lifecycle::WindowFocus;

    #[test]
    fn the_process_registry_projects_lifecycle_and_insets() {
        clear_native_host_facts();
        assert_eq!(native_host_facts(), NativeHostFacts::default());

        let update = push_lifecycle_fact(HostLifecycleFact {
            generation: 3,
            phase: ActivityPhase::Resumed,
            focus: WindowFocus::Focused,
            visible: true,
        });
        assert_eq!(update, LifecycleUpdate::Applied { recreated: true });
        assert!(native_host_facts().lifecycle.surface_active());

        let insets = push_raw_insets(RawInsetsPx::new(
            2.0,
            EdgeInsetsPx::new(48, 0, 96, 0),
            EdgeInsetsPx::ZERO,
        ));
        assert_eq!(insets.safe_area.top, 24.0);
        assert_eq!(native_host_facts().insets, insets);

        clear_native_host_facts();
        assert_eq!(native_host_facts(), NativeHostFacts::default());
    }

    #[test]
    fn the_touch_port_is_unsupported_until_the_host_declares_touch() {
        clear_native_host_facts();
        let port = AndroidTouchGesturePort;
        let unsupported = port
            .touch_gesture_report()
            .now_or_never()
            .expect("default report is ready")
            .expect("default never fails");
        assert!(!unsupported.is_hosted());

        declare_touch_host();
        push_raw_insets(RawInsetsPx::new(
            1.0,
            EdgeInsetsPx::new(30, 0, 60, 0),
            EdgeInsetsPx::ZERO,
        ));
        let hosted = port
            .touch_gesture_report()
            .now_or_never()
            .expect("report is ready")
            .expect("report never fails");
        assert!(hosted.is_hosted());
        assert!(hosted.support.multi_touch());
        assert_eq!(hosted.insets.top, 30.0);
        assert_eq!(hosted.insets.bottom, 60.0);

        clear_native_host_facts();
    }
}
