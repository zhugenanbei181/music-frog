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

    /// Whether the Activity has reached a potentially visible phase. `Paused`
    /// may remain partially visible; `Created` has not reached `onStart` yet.
    pub const fn is_visible(self) -> bool {
        matches!(self, Self::Started | Self::Resumed | Self::Paused)
    }

    /// Apply one legal Android lifecycle transition.
    pub const fn transition_to(self, next: Self) -> ActivityPhaseTransition {
        use ActivityPhase::Created;
        use ActivityPhase::Destroyed;
        use ActivityPhase::Paused;
        use ActivityPhase::Resumed;
        use ActivityPhase::Started;
        use ActivityPhase::Stopped;
        use ActivityPhaseTransition::Advanced;
        use ActivityPhaseTransition::Rejected;
        use ActivityPhaseTransition::Unchanged;

        match self {
            Created => match next {
                Created => Unchanged,
                Started | Destroyed => Advanced,
                Resumed | Paused | Stopped => Rejected,
            },
            Started => match next {
                Started => Unchanged,
                Resumed | Stopped | Destroyed => Advanced,
                Created | Paused => Rejected,
            },
            Resumed => match next {
                Resumed => Unchanged,
                Paused | Stopped | Destroyed => Advanced,
                Created | Started => Rejected,
            },
            Paused => match next {
                Paused => Unchanged,
                Resumed | Stopped | Destroyed => Advanced,
                Created | Started => Rejected,
            },
            Stopped => match next {
                Stopped => Unchanged,
                Started | Destroyed => Advanced,
                Created | Resumed | Paused => Rejected,
            },
            Destroyed => match next {
                Destroyed => Unchanged,
                Created | Started | Resumed | Paused | Stopped => Rejected,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActivityPhaseTransition {
    Unchanged,
    Advanced,
    Rejected,
}

/// Window focus, reported independently of the Activity lifecycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowFocus {
    Unknown,
    Focused,
    Unfocused,
}

impl WindowFocus {
    pub const fn from_code(code: i32) -> Option<Self> {
        match code {
            0 => Some(Self::Unknown),
            1 => Some(Self::Focused),
            2 => Some(Self::Unfocused),
            _ => None,
        }
    }

    pub const fn as_code(self) -> i32 {
        match self {
            Self::Unknown => 0,
            Self::Focused => 1,
            Self::Unfocused => 2,
        }
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
}

/// The host observation is either not installed yet or contains one coherent
/// Activity phase and its independent window-focus fact.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostLifecycleState {
    Unobserved,
    Observed {
        generation: u64,
        phase: ActivityPhase,
        focus: WindowFocus,
    },
}

impl HostLifecycleState {
    /// Whether to observe and render. Before the first host observation, allow
    /// the initial frame without claiming foreground or focus.
    pub const fn surface_active(self) -> bool {
        match self {
            Self::Unobserved => true,
            Self::Observed { phase, .. } => phase.is_visible(),
        }
    }

    /// The shared render cadence these facts select. A paused/stopped/
    /// destroyed or hidden surface is a hard [`RenderCadence::Suspended`]; a
    /// visible but unfocused window is [`RenderCadence::Background`]; a
    /// visible, focused window is [`RenderCadence::Active`].
    pub const fn cadence(self) -> RenderCadence {
        match self {
            Self::Unobserved => RenderCadence::Active,
            Self::Observed { phase, .. } if !phase.is_visible() => RenderCadence::Suspended,
            Self::Observed {
                focus: WindowFocus::Focused,
                ..
            } => RenderCadence::Active,
            Self::Observed {
                focus: WindowFocus::Unknown | WindowFocus::Unfocused,
                ..
            } => RenderCadence::Background,
        }
    }

    /// Whether the Activity is in the foreground.
    pub const fn is_foreground(self) -> bool {
        match self {
            Self::Unobserved => false,
            Self::Observed { phase, .. } => phase.is_foreground(),
        }
    }
}

/// The outcome of applying one lifecycle fact.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LifecycleUpdate {
    /// The first accepted host observation or a legal phase/focus update.
    Applied,
    /// A newer Activity instance replaced the prior generation.
    Recreated,
    /// A fact from a retired generation; it was ignored.
    Stale,
    /// The phase jump is not allowed for this Activity generation.
    Rejected {
        from: ActivityPhase,
        to: ActivityPhase,
    },
}

/// Tracks the latest lifecycle fact and fences Activity recreation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostLifecycleTracker {
    current: HostLifecycleState,
}

impl HostLifecycleTracker {
    pub const fn new() -> Self {
        Self {
            current: HostLifecycleState::Unobserved,
        }
    }

    pub const fn current(&self) -> HostLifecycleState {
        self.current
    }

    /// Apply one fact. Retired generations and illegal same-generation phase
    /// jumps cannot overwrite the current observation.
    pub fn apply(&mut self, fact: HostLifecycleFact) -> LifecycleUpdate {
        let update = match self.current {
            HostLifecycleState::Unobserved => LifecycleUpdate::Applied,
            HostLifecycleState::Observed { generation, .. } if fact.generation < generation => {
                return LifecycleUpdate::Stale;
            }
            HostLifecycleState::Observed { generation, .. } if fact.generation > generation => {
                LifecycleUpdate::Recreated
            }
            HostLifecycleState::Observed { phase, .. } => match phase.transition_to(fact.phase) {
                ActivityPhaseTransition::Unchanged | ActivityPhaseTransition::Advanced => {
                    LifecycleUpdate::Applied
                }
                ActivityPhaseTransition::Rejected => {
                    return LifecycleUpdate::Rejected {
                        from: phase,
                        to: fact.phase,
                    };
                }
            },
        };
        self.current = HostLifecycleState::Observed {
            generation: fact.generation,
            phase: fact.phase,
            focus: fact.focus,
        };
        update
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

    fn fact(generation: u64, phase: ActivityPhase, focus: WindowFocus) -> HostLifecycleFact {
        HostLifecycleFact {
            generation,
            phase,
            focus,
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
        for focus in [
            WindowFocus::Unknown,
            WindowFocus::Focused,
            WindowFocus::Unfocused,
        ] {
            assert_eq!(WindowFocus::from_code(focus.as_code()), Some(focus));
        }
        assert_eq!(ActivityPhase::from_code(-1), None);
        assert_eq!(ActivityPhase::from_code(6), None);
        assert_eq!(WindowFocus::from_code(-1), None);
        assert_eq!(WindowFocus::from_code(3), None);
    }

    #[test]
    fn an_unobserved_host_allows_an_initial_frame_without_foreground() {
        let tracker = HostLifecycleTracker::new();
        assert_eq!(tracker.current(), HostLifecycleState::Unobserved);
        assert!(tracker.current().surface_active());
        assert_eq!(tracker.current().cadence(), RenderCadence::Active);
    }

    #[test]
    fn pause_stops_frames_and_resume_restores_them() {
        let mut tracker = HostLifecycleTracker::new();
        assert_eq!(
            tracker.apply(fact(1, ActivityPhase::Created, WindowFocus::Unknown)),
            LifecycleUpdate::Applied
        );
        assert_eq!(tracker.current().cadence(), RenderCadence::Suspended);
        assert_eq!(
            tracker.apply(fact(1, ActivityPhase::Started, WindowFocus::Unknown)),
            LifecycleUpdate::Applied
        );
        assert_eq!(tracker.current().cadence(), RenderCadence::Background);
        assert_eq!(
            tracker.apply(fact(1, ActivityPhase::Started, WindowFocus::Focused)),
            LifecycleUpdate::Applied
        );
        assert_eq!(tracker.current().cadence(), RenderCadence::Active);
        assert_eq!(
            tracker.apply(fact(1, ActivityPhase::Resumed, WindowFocus::Focused)),
            LifecycleUpdate::Applied
        );
        assert_eq!(tracker.current().cadence(), RenderCadence::Active);

        tracker.apply(fact(1, ActivityPhase::Paused, WindowFocus::Unfocused));
        assert!(!tracker.current().is_foreground());
        // Still visible (paused), but unfocused: the 2 FPS background rate.
        assert_eq!(tracker.current().cadence(), RenderCadence::Background);

        tracker.apply(fact(1, ActivityPhase::Stopped, WindowFocus::Unfocused));
        assert!(!tracker.current().surface_active());
        assert_eq!(tracker.current().cadence(), RenderCadence::Suspended);

        tracker.apply(fact(1, ActivityPhase::Started, WindowFocus::Unfocused));
        assert_eq!(tracker.current().cadence(), RenderCadence::Background);
        tracker.apply(fact(1, ActivityPhase::Resumed, WindowFocus::Focused));
        assert_eq!(tracker.current().cadence(), RenderCadence::Active);
    }

    #[test]
    fn focus_is_independent_of_the_lifecycle_phase() {
        let mut tracker = HostLifecycleTracker::new();
        // Resumed but the window lost focus (a dialog/split screen): the
        // surface stays visible and drops to the background cadence.
        tracker.apply(fact(1, ActivityPhase::Resumed, WindowFocus::Unfocused));
        assert!(tracker.current().surface_active());
        assert!(tracker.current().is_foreground());
        assert_eq!(tracker.current().cadence(), RenderCadence::Background);
    }

    #[test]
    fn a_retired_activity_cannot_overwrite_the_recreated_one() {
        let mut tracker = HostLifecycleTracker::new();
        assert_eq!(
            tracker.apply(fact(7, ActivityPhase::Created, WindowFocus::Unknown)),
            LifecycleUpdate::Applied
        );
        assert_eq!(
            tracker.apply(fact(7, ActivityPhase::Started, WindowFocus::Focused)),
            LifecycleUpdate::Applied
        );
        assert_eq!(
            tracker.apply(fact(7, ActivityPhase::Resumed, WindowFocus::Focused)),
            LifecycleUpdate::Applied
        );
        // A late callback from the previous Activity instance is rejected.
        assert_eq!(
            tracker.apply(fact(6, ActivityPhase::Stopped, WindowFocus::Unfocused)),
            LifecycleUpdate::Stale
        );
        assert_eq!(
            tracker.current(),
            HostLifecycleState::Observed {
                generation: 7,
                phase: ActivityPhase::Resumed,
                focus: WindowFocus::Focused,
            }
        );
        assert!(tracker.current().surface_active());

        // The recreated Activity is reported as a recreation.
        assert_eq!(
            tracker.apply(fact(8, ActivityPhase::Created, WindowFocus::Unknown)),
            LifecycleUpdate::Recreated
        );
        assert_eq!(
            tracker.current(),
            HostLifecycleState::Observed {
                generation: 8,
                phase: ActivityPhase::Created,
                focus: WindowFocus::Unknown,
            }
        );
    }

    #[test]
    fn illegal_phase_jump_is_rejected_without_changing_the_previous_state() {
        let mut tracker = HostLifecycleTracker::new();
        tracker.apply(fact(2, ActivityPhase::Created, WindowFocus::Unknown));
        assert_eq!(
            tracker.apply(fact(2, ActivityPhase::Paused, WindowFocus::Focused)),
            LifecycleUpdate::Rejected {
                from: ActivityPhase::Created,
                to: ActivityPhase::Paused,
            }
        );
        assert_eq!(
            tracker.current(),
            HostLifecycleState::Observed {
                generation: 2,
                phase: ActivityPhase::Created,
                focus: WindowFocus::Unknown,
            }
        );
    }

    #[test]
    fn destroyed_activity_is_terminal_for_its_generation() {
        let mut tracker = HostLifecycleTracker::new();
        tracker.apply(fact(3, ActivityPhase::Created, WindowFocus::Unknown));
        tracker.apply(fact(3, ActivityPhase::Started, WindowFocus::Focused));
        tracker.apply(fact(3, ActivityPhase::Destroyed, WindowFocus::Unknown));
        assert_eq!(
            tracker.apply(fact(3, ActivityPhase::Started, WindowFocus::Focused)),
            LifecycleUpdate::Rejected {
                from: ActivityPhase::Destroyed,
                to: ActivityPhase::Started,
            }
        );
        assert!(!tracker.current().surface_active());
        assert_eq!(tracker.current().cadence(), RenderCadence::Suspended);
    }
}
