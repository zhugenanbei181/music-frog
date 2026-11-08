//! Projects the owner's runtime readback into the mascot's typed motion.
//!
//! A surface never asks "is the frog playing?". The lifecycle readback and a
//! typed cue pass through one state machine here; the same function serves the
//! live window and the deterministic capture frame.

use infiltrator_contract::mascot_motion::{
    MascotMotion, MascotMotionSpec, MotionCue, STILL_FRAME, frame,
};
use lurker_core_contract::lifecycle::CoreLifecycle;

/// The target of an intent the owner has not confirmed yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MascotTarget {
    Running,
    Stopped,
}

/// Whether the frog waits for the owner to echo a just-dispatched intent.
/// A stale readback from before the command must not cancel the flourish, and
/// an intent the owner never confirms must not animate forever: both are
/// expressed here, never as a pair of booleans in a view.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum MotionSettlement {
    #[default]
    Confirmed,
    Awaiting {
        target: MascotTarget,
        since_millis: u64,
    },
}

/// How long a flourish may wait for the owner before the readback wins.
pub const SETTLEMENT_TIMEOUT_MILLIS: u64 = 1_500;

/// The one mutable fact of the animation: which state the frog is in, when it
/// entered it, and whether it waits for a readback. There is no second
/// `is_playing` bit in any surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct MascotMotionState {
    pub motion: MascotMotion,
    pub entered_at_millis: u64,
    pub settlement: MotionSettlement,
}

impl MascotMotionState {
    /// Apply one user intent. The owner has not answered yet, so the flourish
    /// starts now and the settlement is explicitly pending.
    fn request(&mut self, cue: MotionCue, now_millis: u64) {
        let target = match cue {
            MotionCue::StartRequested | MotionCue::Settled { running: true } => {
                MascotTarget::Running
            }
            MotionCue::StopRequested | MotionCue::Settled { running: false } => {
                MascotTarget::Stopped
            }
            MotionCue::ReducedMotion { .. } => return,
        };
        let next = self.motion.transition(cue);
        if next != self.motion {
            self.motion = next;
            self.entered_at_millis = now_millis;
        }
        self.settlement = MotionSettlement::Awaiting {
            target,
            since_millis: now_millis,
        };
    }

    fn settle(&mut self, running: bool, now_millis: u64) {
        let cue = MotionCue::Settled { running };
        let next = self.motion.transition(cue);
        if next != self.motion {
            self.motion = next;
            self.entered_at_millis = now_millis;
        }
        self.settlement = MotionSettlement::Confirmed;
    }

    /// Resolve the frame. A capture host or a reduced-motion preference gets
    /// the still pose, so no receipt ever races a beat.
    pub fn spec(&self, now_millis: u64, frozen: bool) -> MascotMotionSpec {
        if frozen {
            return MascotMotionSpec {
                state: self.motion,
                control_glyph: self.motion.glyph(),
                ..STILL_FRAME
            };
        }
        let elapsed = now_millis.saturating_sub(self.entered_at_millis);
        frame(self.motion, elapsed.min(u32::MAX as u64) as u32, false)
    }
}

/// The owner's lifecycle is the only authority on whether the frog runs.
pub fn cue_for_lifecycle(lifecycle: CoreLifecycle) -> MotionCue {
    let running = matches!(lifecycle, CoreLifecycle::Ready | CoreLifecycle::Running);
    MotionCue::Settled { running }
}

/// A full projection step: readback first (authoritative), then the user's
/// intent, then one frame. Keeping the order here prevents a surface from
/// racing a pending command against a stale snapshot.
pub fn project_mascot(
    state: &mut MascotMotionState,
    lifecycle: CoreLifecycle,
    cue: Option<MotionCue>,
    now_millis: u64,
    frozen: bool,
) -> MascotMotionSpec {
    let running = matches!(lifecycle, CoreLifecycle::Ready | CoreLifecycle::Running);
    match cue {
        Some(intent @ (MotionCue::StartRequested | MotionCue::StopRequested)) => {
            state.request(intent, now_millis)
        }
        Some(MotionCue::ReducedMotion { .. }) => {}
        Some(MotionCue::Settled { running: confirmed }) => state.settle(confirmed, now_millis),
        None => {}
    }
    match state.settlement {
        MotionSettlement::Awaiting {
            target,
            since_millis,
        } => {
            let matches = matches!(
                (target, running),
                (MascotTarget::Running, true) | (MascotTarget::Stopped, false)
            );
            if matches || now_millis.saturating_sub(since_millis) >= SETTLEMENT_TIMEOUT_MILLIS {
                state.settle(running, now_millis);
            }
        }
        MotionSettlement::Confirmed => state.settle(running, now_millis),
    }
    state.spec(now_millis, frozen)
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::mascot_motion::HeroGlyph;

    #[test]
    fn a_start_tap_then_owner_readback_moves_the_frog_through_a_flourish() {
        let mut state = MascotMotionState::default();
        let starting = project_mascot(
            &mut state,
            CoreLifecycle::Starting,
            Some(MotionCue::StartRequested),
            0,
            false,
        );
        assert_eq!(starting.state, MascotMotion::Starting);
        assert_eq!(starting.control_glyph, HeroGlyph::Pause);
        assert!(starting.note_count > 0, "a tap releases notes immediately");

        let running = project_mascot(&mut state, CoreLifecycle::Running, None, 500, false);
        assert_eq!(running.state, MascotMotion::Running);
        assert_eq!(running.control_glyph, HeroGlyph::Pause);
    }

    #[test]
    fn a_stop_tap_settles_to_rest_and_the_notes_clear() {
        let mut state = MascotMotionState {
            motion: MascotMotion::Running,
            entered_at_millis: 0,
            settlement: MotionSettlement::Confirmed,
        };
        let stopping = project_mascot(
            &mut state,
            CoreLifecycle::Running,
            Some(MotionCue::StopRequested),
            900,
            false,
        );
        assert_eq!(stopping.state, MascotMotion::Stopping);
        assert_eq!(stopping.control_glyph, HeroGlyph::Play);
        let idle = project_mascot(&mut state, CoreLifecycle::Stopped, None, 1_400, false);
        assert_eq!(idle, STILL_FRAME);
    }

    #[test]
    fn a_stale_readback_does_not_cancel_a_flourish_but_a_timeout_returns_to_rest() {
        let mut state = MascotMotionState::default();
        state.request(MotionCue::StartRequested, 100);
        // The snapshot still says Stopped: the command has not landed yet.
        let flourish = project_mascot(&mut state, CoreLifecycle::Stopped, None, 120, false);
        assert_eq!(flourish.state, MascotMotion::Starting);
        assert!(flourish.note_count > 0, "the tap is answered immediately");
        // The owner never confirms. The typed settlement expires and rest wins.
        let settled = project_mascot(
            &mut state,
            CoreLifecycle::Stopped,
            None,
            100 + SETTLEMENT_TIMEOUT_MILLIS,
            false,
        );
        assert_eq!(settled, STILL_FRAME);
        assert_eq!(state.settlement, MotionSettlement::Confirmed);
    }

    #[test]
    fn a_frozen_host_keeps_the_state_but_holds_the_still_pose() {
        let mut state = MascotMotionState {
            motion: MascotMotion::Running,
            entered_at_millis: 0,
            settlement: MotionSettlement::Confirmed,
        };
        let first = project_mascot(&mut state, CoreLifecycle::Running, None, 111, true);
        let second = project_mascot(&mut state, CoreLifecycle::Running, None, 9_999, true);
        assert_eq!(first, second);
        assert_eq!(first.control_glyph, HeroGlyph::Pause);
        assert_eq!(first.note_count, 0);
    }
}
