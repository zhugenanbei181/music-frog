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
pub mod configuration;
pub mod gesture;
pub mod ime;
pub mod insets;
pub mod lifecycle;
pub mod surface;

use async_trait::async_trait;
use configuration::ConfigurationChange;
use configuration::ConfigurationFact;
use configuration::ConfigurationTracker;
use configuration::HostConfigurationState;
use gesture::GestureArbitrator;
use gesture::GestureOutcome;
use gesture::GestureSample;
use insets::NativeInsets;
use insets::RawInsetsPx;
use lifecycle::HostLifecycleFact;
use lifecycle::HostLifecycleState;
use lifecycle::HostLifecycleTracker;
use lifecycle::LifecycleUpdate;
use std::sync::Mutex;
use std::sync::OnceLock;
use surface::SurfaceFact;
use surface::SurfaceLifecycleState;
use surface::SurfaceLifecycleTracker;
use surface::SurfaceUpdate;

use infiltrator_ports::touch_gesture::TouchGestureHostReport;
use infiltrator_ports::touch_gesture::TouchGesturePort;
use lurker_core_ports::error::PortError;

/// The typed host facts the shared shell consumes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeHostFacts {
    pub lifecycle: HostLifecycleState,
    pub insets: NativeInsets,
    /// EFFECTS-047 (#190): the render surface the Activity owns.
    pub surface: SurfaceLifecycleState,
    /// EFFECTS-047 (#192): the window shape the Activity published.
    pub configuration: HostConfigurationState,
}

impl Default for NativeHostFacts {
    fn default() -> Self {
        Self {
            lifecycle: HostLifecycleState::Unobserved,
            insets: NativeInsets::ZERO,
            surface: SurfaceLifecycleState::Unobserved,
            configuration: HostConfigurationState::Unobserved,
        }
    }
}

struct HostFactState {
    lifecycle: HostLifecycleTracker,
    insets: NativeInsets,
    touch_declared: bool,
    surface: SurfaceLifecycleTracker,
    configuration: ConfigurationTracker,
    gesture: GestureArbitrator,
}

impl HostFactState {
    fn new() -> Self {
        Self {
            lifecycle: HostLifecycleTracker::new(),
            insets: NativeInsets::ZERO,
            touch_declared: false,
            surface: SurfaceLifecycleTracker::new(),
            configuration: ConfigurationTracker::new(),
            gesture: GestureArbitrator::default(),
        }
    }

    fn snapshot(&self) -> NativeHostFacts {
        NativeHostFacts {
            lifecycle: self.lifecycle.current(),
            insets: self.insets,
            surface: self.surface.current(),
            configuration: self.configuration.current(),
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

/// Push one lifecycle fact from the native Activity. Typed results distinguish
/// accepted observations/recreation from stale generations and illegal jumps.
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

/// EFFECTS-047 (#190): push one surface fact from the native Activity.
///
/// The Activity generation fences a retired instance: a late callback from a
/// destroyed Activity can never overwrite the live surface state.
pub fn push_surface_fact(fact: SurfaceFact) -> SurfaceUpdate {
    with_state(|state| state.surface.apply(fact))
}

/// EFFECTS-047 (#192): push one window configuration fact from the native
/// Activity. The returned change tells the shell whether it must relayout.
pub fn push_configuration_fact(fact: ConfigurationFact) -> ConfigurationChange {
    with_state(|state| state.configuration.apply(fact))
}

/// EFFECTS-047 (#191): push one normalized pointer sample and read the typed
/// arbitration verdict. The arbitration rule lives here, so the surface only
/// consumes the verdict.
pub fn push_gesture_sample(sample: GestureSample) -> GestureOutcome {
    with_state(|state| state.gesture.apply(sample))
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
        state.surface = SurfaceLifecycleTracker::new();
        state.configuration = ConfigurationTracker::new();
        state.gesture = GestureArbitrator::default();
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
        let (declared, insets, surface) =
            with_state(|state| (state.touch_declared, state.insets, state.surface.current()));
        if !declared {
            return Ok(TouchGestureHostReport::default());
        }
        // A torn down surface has no touch surface to host a gesture on, so the
        // host reports the typed unsupported boundary instead of pretending.
        if surface.is_torn_down() {
            return Ok(TouchGestureHostReport::unsupported(
                "the Android host surface was torn down",
            ));
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
            phase: ActivityPhase::Created,
            focus: WindowFocus::Unknown,
        });
        assert_eq!(update, LifecycleUpdate::Applied);
        push_lifecycle_fact(HostLifecycleFact {
            generation: 3,
            phase: ActivityPhase::Started,
            focus: WindowFocus::Unknown,
        });
        push_lifecycle_fact(HostLifecycleFact {
            generation: 3,
            phase: ActivityPhase::Resumed,
            focus: WindowFocus::Focused,
        });
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

    #[test]
    fn the_surface_fact_is_projected_and_fences_retired_activities() {
        clear_native_host_facts();

        assert_eq!(
            push_surface_fact(SurfaceFact::new(
                2,
                surface::HostSurfaceState::Live {
                    width_px: 1080,
                    height_px: 1920,
                },
            )),
            SurfaceUpdate::Applied
        );
        assert_eq!(
            native_host_facts().surface,
            SurfaceLifecycleState::Live {
                generation: 2,
                width_px: 1080,
                height_px: 1920,
            }
        );

        // A retired Activity cannot tear down the live surface.
        assert_eq!(
            push_surface_fact(SurfaceFact::new(1, surface::HostSurfaceState::Destroyed)),
            SurfaceUpdate::Stale
        );
        assert!(native_host_facts().surface.can_render());

        assert_eq!(
            push_surface_fact(SurfaceFact::new(2, surface::HostSurfaceState::Destroyed)),
            SurfaceUpdate::Applied
        );
        assert!(native_host_facts().surface.is_torn_down());
        assert!(!native_host_facts().surface.can_render());

        clear_native_host_facts();
        assert_eq!(
            native_host_facts().surface,
            SurfaceLifecycleState::Unobserved
        );
    }

    #[test]
    fn the_configuration_fact_reports_the_rotation_that_requires_a_relayout() {
        clear_native_host_facts();

        assert_eq!(
            push_configuration_fact(ConfigurationFact::new(
                1,
                configuration::HostOrientation::Portrait,
                1080,
                1920,
                2.75,
                1.0,
            )),
            ConfigurationChange::Unchanged
        );
        assert_eq!(
            push_configuration_fact(ConfigurationFact::new(
                1,
                configuration::HostOrientation::Landscape,
                1920,
                1080,
                2.75,
                1.0,
            )),
            ConfigurationChange::OrientationChanged {
                from: configuration::HostOrientation::Portrait,
                to: configuration::HostOrientation::Landscape,
            }
        );
        assert!(matches!(
            native_host_facts().configuration,
            HostConfigurationState::Configured(_)
        ));

        clear_native_host_facts();
        assert_eq!(
            native_host_facts().configuration,
            HostConfigurationState::Unobserved
        );
    }

    #[test]
    fn the_gesture_fact_answers_with_the_typed_arbitration_verdict() {
        clear_native_host_facts();

        assert_eq!(
            push_gesture_sample(GestureSample::new(
                1,
                gesture::TouchPhase::Started,
                100.0,
                200.0,
                0,
            )),
            GestureOutcome::Undecided
        );
        assert_eq!(
            push_gesture_sample(GestureSample::new(
                1,
                gesture::TouchPhase::Ended,
                100.0,
                260.0,
                90,
            )),
            GestureOutcome::Scroll {
                direction: gesture::ScrollDirection::Vertical,
            }
        );
        assert!(
            GestureOutcome::Scroll {
                direction: gesture::ScrollDirection::Vertical,
            }
            .blocks_row_action()
        );

        clear_native_host_facts();
    }
}
