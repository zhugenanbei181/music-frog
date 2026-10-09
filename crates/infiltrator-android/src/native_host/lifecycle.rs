//! BANDROID-005: native Activity lifecycle facts projected onto the shared
//! render-cadence seam.
//!
//! The Kotlin host owns the real `onCreate`/`onStart`/`onResume`/`onPause`/
//! `onStop`/`onDestroy` and window-focus callbacks. This module turns those
//! facts into the toolkit-neutral [`RenderCadence`] the shared shell already
//! consumes, and fences Activity recreation with a monotonic generation so a
//! retired Activity cannot overwrite the live one's state.
//!
//! Android delivers lifecycle and window focus as *separate* facts: a window
//! can lose focus while the Activity stays resumed (dialogs, split screen), so
//! focus is never inferred from the lifecycle phase.

use infiltrator_contract::cadence::RenderCadence;

/// The Android Activity lifecycle phase as reported by the native host.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActivityPhase {
    Created,
    Started,
    Resumed,
    Paused,
    Stopped,
    Destroyed,
}

impl ActivityPhase {
    /// Decode the wire code the Kotlin host sends over JNI.
    pub const fn from_code(code: i32) -> Option<Self> {
        match code {
            0 => Some(Self::Created),
            1 => Some(Self::Started),
            2 => Some(Self::Resumed),
            3 => Some(Self::Paused),
            4 => Some(Self::Stopped),
            5 => Some(Self::Destroyed),
            _ => None,
        }
    }

    /// The wire code for this phase (the Kotlin constants mirror these).
    pub const fn as_code(self) -> i32 {
        match self {
            Self::Created => 0,
            Self::Started => 1,
            Self::Resumed => 2,
            Self::Paused => 3,
            Self::Stopped => 4,
            Self::Destroyed => 5,
        }
    }

    /// Whether the Activity is in the foreground (`onResume` delivered).
    pub const fn is_foreground(self) -> bool {
        matches!(self, Self::Resumed)
    }

    /// Whether the Activity can still be visible to the user. `Paused` is
    /// partially obscured but visible; only `Stopped`/`Destroyed` hide it.
    pub const fn is_visible(self) -> bool {
        matches!(
            self,
            Self::Created | Self::Started | Self::Resumed | Self::Paused
        )
    }
}

/// Window focus, reported independently of the Activity lifecycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowFocus {
    Focused,
    Unfocused,
}

impl WindowFocus {
    pub const fn is_focused(self) -> bool {
        matches!(self, Self::Focused)
    }
}

/// One raw lifecycle observation pushed by the native Activity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostLifecycleFact {
    /// Monotonic per-Activity-instance generation; a recreated Activity has a
    /// strictly larger value than the instance it replaced.
    pub generation: u64,
    pub phase: ActivityPhase,
    pub focus: WindowFocus,
    /// Whether the host reports the Activity started (`onStart`..`onStop`).
    pub visible: bool,
}

/// The typed lifecycle state the UI consumes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostLifecycleState {
    pub generation: u64,
    pub phase: ActivityPhase,
    pub focus: WindowFocus,
    pub visible: bool,
}

impl HostLifecycleState {
    /// The state before the host has pushed anything: treated as an active,
    /// focused, visible surface so a cold start is never throttled by a
    /// missing observation.
    pub const COLD: Self = Self {
        generation: 0,
        phase: ActivityPhase::Created,
        focus: WindowFocus::Focused,
        visible: true,
    };

    /// Whether the surface should observe and render at all.
    pub const fn surface_active(self) -> bool {
        self.visible && self.phase.is_visible() && !matches!(self.phase, ActivityPhase::Destroyed)
    }

    /// The shared render cadence these facts select. A paused/stopped/
    /// destroyed or hidden surface is a hard [`RenderCadence::Suspended`]; a
    /// visible but unfocused window is [`RenderCadence::Background`]; a
    /// visible, focused window is [`RenderCadence::Active`].
    pub const fn cadence(self) -> RenderCadence {
        if !self.surface_active() {
            return RenderCadence::Suspended;
        }
        RenderCadence::from_visible_focused(true, self.focus.is_focused())
    }

    /// Whether the Activity is in the foreground.
    pub const fn is_foreground(self) -> bool {
        self.surface_active() && self.phase.is_foreground()
    }
}

/// The outcome of applying one lifecycle fact.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LifecycleUpdate {
    /// The fact was newer or equal and replaced the state; `recreated` marks a
    /// generation bump (the previous Activity's requests must be retired).
    Applied { recreated: bool },
    /// A fact from a retired generation; it was ignored.
    Stale,
}

/// Tracks the latest lifecycle fact and fences Activity recreation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostLifecycleTracker {
    current: HostLifecycleState,
}

impl HostLifecycleTracker {
    pub const fn new() -> Self {
        Self {
            current: HostLifecycleState::COLD,
        }
    }

    pub const fn current(&self) -> HostLifecycleState {
        self.current
    }

    /// Apply one fact. A fact from an older generation is rejected so a
    /// retiring Activity cannot overwrite the recreated one.
    pub fn apply(&mut self, fact: HostLifecycleFact) -> LifecycleUpdate {
        if fact.generation < self.current.generation {
            return LifecycleUpdate::Stale;
        }
        let recreated = fact.generation > self.current.generation;
        self.current = HostLifecycleState {
            generation: fact.generation,
            phase: fact.phase,
            focus: fact.focus,
            visible: fact.visible && fact.phase.is_visible(),
        };
        LifecycleUpdate::Applied { recreated }
    }
}

impl Default for HostLifecycleTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fact(
        generation: u64,
        phase: ActivityPhase,
        focus: WindowFocus,
        visible: bool,
    ) -> HostLifecycleFact {
        HostLifecycleFact {
            generation,
            phase,
            focus,
            visible,
        }
    }

    #[test]
    fn wire_codes_round_trip_and_unknown_is_rejected() {
        for phase in [
            ActivityPhase::Created,
            ActivityPhase::Started,
            ActivityPhase::Resumed,
            ActivityPhase::Paused,
            ActivityPhase::Stopped,
            ActivityPhase::Destroyed,
        ] {
            assert_eq!(ActivityPhase::from_code(phase.as_code()), Some(phase));
        }
        assert_eq!(ActivityPhase::from_code(-1), None);
        assert_eq!(ActivityPhase::from_code(6), None);
    }

    #[test]
    fn a_cold_host_starts_active_and_focused() {
        let tracker = HostLifecycleTracker::new();
        assert_eq!(tracker.current(), HostLifecycleState::COLD);
        assert!(tracker.current().surface_active());
        assert_eq!(tracker.current().cadence(), RenderCadence::Active);
    }

    #[test]
    fn pause_stops_frames_and_resume_restores_them() {
        let mut tracker = HostLifecycleTracker::new();
        assert_eq!(
            tracker.apply(fact(1, ActivityPhase::Resumed, WindowFocus::Focused, true)),
            LifecycleUpdate::Applied { recreated: true }
        );
        assert_eq!(tracker.current().cadence(), RenderCadence::Active);

        tracker.apply(fact(1, ActivityPhase::Paused, WindowFocus::Unfocused, true));
        assert!(!tracker.current().is_foreground());
        // Still visible (paused), but unfocused: the 2 FPS background rate.
        assert_eq!(tracker.current().cadence(), RenderCadence::Background);

        tracker.apply(fact(
            1,
            ActivityPhase::Stopped,
            WindowFocus::Unfocused,
            false,
        ));
        assert!(!tracker.current().surface_active());
        assert_eq!(tracker.current().cadence(), RenderCadence::Suspended);

        tracker.apply(fact(1, ActivityPhase::Resumed, WindowFocus::Focused, true));
        assert_eq!(tracker.current().cadence(), RenderCadence::Active);
    }

    #[test]
    fn focus_is_independent_of_the_lifecycle_phase() {
        let mut tracker = HostLifecycleTracker::new();
        // Resumed but the window lost focus (a dialog/split screen): the
        // surface stays visible and drops to the background cadence.
        tracker.apply(fact(
            1,
            ActivityPhase::Resumed,
            WindowFocus::Unfocused,
            true,
        ));
        assert!(tracker.current().surface_active());
        assert!(tracker.current().is_foreground());
        assert_eq!(tracker.current().cadence(), RenderCadence::Background);
    }

    #[test]
    fn a_retired_activity_cannot_overwrite_the_recreated_one() {
        let mut tracker = HostLifecycleTracker::new();
        assert_eq!(
            tracker.apply(fact(7, ActivityPhase::Resumed, WindowFocus::Focused, true)),
            LifecycleUpdate::Applied { recreated: true }
        );
        // A late callback from the previous Activity instance is rejected.
        assert_eq!(
            tracker.apply(fact(
                6,
                ActivityPhase::Stopped,
                WindowFocus::Unfocused,
                false
            )),
            LifecycleUpdate::Stale
        );
        assert_eq!(tracker.current().generation, 7);
        assert!(tracker.current().surface_active());

        // The recreated Activity is reported as a recreation.
        assert_eq!(
            tracker.apply(fact(8, ActivityPhase::Created, WindowFocus::Focused, true)),
            LifecycleUpdate::Applied { recreated: true }
        );
        assert_eq!(tracker.current().generation, 8);
    }

    #[test]
    fn a_hidden_phase_never_claims_a_visible_surface() {
        let mut tracker = HostLifecycleTracker::new();
        // Even if the host wrongly reports `visible = true` for a destroyed
        // Activity, the phase wins and the surface is suspended.
        tracker.apply(fact(
            2,
            ActivityPhase::Destroyed,
            WindowFocus::Unfocused,
            true,
        ));
        assert!(!tracker.current().visible);
        assert!(!tracker.current().surface_active());
        assert_eq!(tracker.current().cadence(), RenderCadence::Suspended);
    }
}
