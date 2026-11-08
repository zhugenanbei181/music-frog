//! The MusicFrog mascot's motion contract (MUSICFROG-001).
//!
//! The frog is the product's identity, not a decoration. Its movement is a
//! business fact with one owner: a surface never owns a second `bool`, a
//! private timer or a "is playing" flag. It consumes the typed [`MascotMotion`]
//! state machine and the derived [`MascotMotionSpec`], and reports typed
//! [`MotionCue`]s back to the projection.
//!
//! The phase is a pure function of the state and elapsed time, so a headless
//! capture can freeze the frog on a deterministic frame while the live window
//! still animates. Reducing motion is a typed cue, not a UI-side shortcut.

use serde::{Deserialize, Serialize};
use std::f32::consts::{PI, TAU};

/// What the mascot is doing. Mutually exclusive states, never a set of flags.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum MascotMotion {
    /// The core is stopped: the frog rests, no notes are emitted.
    #[default]
    Idle,
    /// The user asked to start: one strike of the strings, then it settles.
    Starting,
    /// The core is running: the frog sways to a beat and releases notes.
    Running,
    /// The user asked to stop: one closing flourish, then rest.
    Stopping,
}

/// What happened. A cue is an intent or a fact, never a derived state flag.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MotionCue {
    /// A typed owner intent to start the runtime was acknowledged.
    StartRequested,
    /// A typed owner intent to stop the runtime was acknowledged.
    StopRequested,
    /// The runtime readback confirms the requested state.
    Settled { running: bool },
    /// The host asks for reduced motion. The frog holds a still, readable pose.
    ReducedMotion { reduced: bool },
}

/// The one projection a surface is allowed to draw from.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct MascotMotionSpec {
    pub state: MascotMotion,
    /// Sway in the range -1.0..=1.0.
    pub sway: f32,
    /// Scale pulse in the range 0.0..=1.0.
    pub pulse: f32,
    /// How many notes are visible in the current beat (0..=NOTE_SLOTS).
    pub note_count: u8,
    /// Per-note progress in the range 0.0..=1.0, fixed-size so no surface
    /// allocates or invents its own emitter geometry.
    pub note_progress: [f32; 3],
    /// The glyph the hero control overlays on the mascot.
    pub control_glyph: HeroGlyph,
}

/// The control's vector, still the honest run/stop affordance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HeroGlyph {
    Play,
    Pause,
}

impl MascotMotion {
    pub const ALL: [Self; 4] = [Self::Idle, Self::Starting, Self::Running, Self::Stopping];

    /// Whether this state is a transient flourish that must return to rest.
    pub const fn is_transient(self) -> bool {
        matches!(self, Self::Starting | Self::Stopping)
    }

    /// Typed transition. Every (state, cue) pair is decided here; a surface
    /// never infers a transition from a combination of flags.
    pub const fn transition(self, cue: MotionCue) -> Self {
        match (self, cue) {
            (Self::Idle, MotionCue::StartRequested) => Self::Starting,
            (Self::Running, MotionCue::StopRequested) => Self::Stopping,
            (Self::Starting, MotionCue::StopRequested) => Self::Stopping,
            (Self::Stopping, MotionCue::StartRequested) => Self::Starting,
            (Self::Starting, MotionCue::Settled { running: true }) => Self::Running,
            (Self::Stopping, MotionCue::Settled { running: false }) => Self::Idle,
            (_, MotionCue::Settled { running: true }) => Self::Running,
            (_, MotionCue::Settled { running: false }) => Self::Idle,
            (state, MotionCue::ReducedMotion { .. }) => state,
            // A repeated request in the state it already names is a no-op:
            // the frog does not restart its flourish on every tap.
            (Self::Idle, MotionCue::StopRequested)
            | (Self::Starting, MotionCue::StartRequested)
            | (Self::Running, MotionCue::StartRequested)
            | (Self::Stopping, MotionCue::StopRequested) => self,
        }
    }

    /// The glyph belongs to the authority, not to the surface: while a
    /// transient flourish plays the control already shows the target state.
    pub const fn glyph(self) -> HeroGlyph {
        match self {
            Self::Idle | Self::Stopping => HeroGlyph::Play,
            Self::Starting | Self::Running => HeroGlyph::Pause,
        }
    }
}

/// How many note emitters one beat owns.
pub const NOTE_SLOTS: usize = 3;
/// One full beat, in milliseconds.
pub const BEAT_MILLIS: u32 = 900;
/// A transient flourish, in milliseconds.
pub const FLOURISH_MILLIS: u32 = 420;

/// The frozen frame used by receipts and reduced-motion hosts.
pub const STILL_FRAME: MascotMotionSpec = MascotMotionSpec {
    state: MascotMotion::Idle,
    sway: 0.0,
    pulse: 0.0,
    note_count: 0,
    note_progress: [0.0; NOTE_SLOTS],
    control_glyph: HeroGlyph::Play,
};

/// Resolve one frame. `elapsed_millis` is the time since the state was
/// entered; it is supplied by the host's clock, never measured by the surface.
pub fn frame(state: MascotMotion, elapsed_millis: u32, reduced_motion: bool) -> MascotMotionSpec {
    if reduced_motion || state == MascotMotion::Idle {
        return MascotMotionSpec {
            state,
            control_glyph: state.glyph(),
            ..STILL_FRAME
        };
    }
    let beat = (elapsed_millis % BEAT_MILLIS) as f32 / BEAT_MILLIS as f32;
    let phase = beat * TAU;
    let flourish = if state.is_transient() {
        let progress =
            (elapsed_millis.min(FLOURISH_MILLIS) as f32 / FLOURISH_MILLIS as f32).clamp(0.0, 1.0);
        (progress * PI).sin()
    } else {
        0.0
    };
    let (sway, pulse, note_count) = match state {
        MascotMotion::Idle => (0.0, 0.0, 0),
        MascotMotion::Starting => (0.35 * flourish * phase.sin(), flourish, 2),
        MascotMotion::Running => (phase.sin(), 0.5 + 0.5 * phase.sin(), 3),
        MascotMotion::Stopping => (0.25 * flourish * phase.sin(), 0.6 * flourish, 1),
    };
    let mut note_progress = [0.0; NOTE_SLOTS];
    for (index, slot) in note_progress.iter_mut().enumerate() {
        let offset = index as f32 / NOTE_SLOTS as f32;
        *slot = (beat + offset) % 1.0;
    }
    MascotMotionSpec {
        state,
        sway,
        pulse,
        note_count,
        note_progress,
        control_glyph: state.glyph(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_state_and_cue_transition_is_decided() {
        for state in MascotMotion::ALL {
            for cue in [
                MotionCue::StartRequested,
                MotionCue::StopRequested,
                MotionCue::Settled { running: true },
                MotionCue::Settled { running: false },
                MotionCue::ReducedMotion { reduced: true },
                MotionCue::ReducedMotion { reduced: false },
            ] {
                let next = state.transition(cue);
                assert!(MascotMotion::ALL.contains(&next));
                if let MotionCue::Settled { running } = cue {
                    assert_eq!(
                        next == MascotMotion::Running,
                        running,
                        "settled readback is authoritative"
                    );
                }
            }
        }
    }

    #[test]
    fn a_start_flourish_settles_into_running_and_stop_settles_into_idle() {
        let starting = MascotMotion::Idle.transition(MotionCue::StartRequested);
        assert_eq!(starting, MascotMotion::Starting);
        let running = starting.transition(MotionCue::Settled { running: true });
        assert_eq!(running, MascotMotion::Running);
        let stopping = running.transition(MotionCue::StopRequested);
        assert_eq!(stopping, MascotMotion::Stopping);
        assert_eq!(
            stopping.transition(MotionCue::Settled { running: false }),
            MascotMotion::Idle
        );
    }

    #[test]
    fn reduced_motion_and_capture_hold_a_deterministic_still_frame() {
        let still = frame(MascotMotion::Running, 12_345, true);
        assert_eq!(
            still,
            MascotMotionSpec {
                control_glyph: HeroGlyph::Pause,
                state: MascotMotion::Running,
                ..STILL_FRAME
            }
        );
        assert_eq!(frame(MascotMotion::Idle, 0, false), STILL_FRAME);
    }

    #[test]
    fn a_running_beat_is_bounded_periodic_and_never_negative() {
        for millis in [0, 1, 111, 450, 899, 900, 1_801, u32::MAX] {
            let spec = frame(MascotMotion::Running, millis, false);
            assert!((-1.0..=1.0).contains(&spec.sway), "sway at {millis}");
            assert!((0.0..=1.0).contains(&spec.pulse), "pulse at {millis}");
            assert_eq!(spec.note_count, NOTE_SLOTS as u8);
            for progress in spec.note_progress {
                assert!((0.0..=1.0).contains(&progress));
            }
        }
        assert_eq!(
            frame(MascotMotion::Running, 0, false),
            frame(MascotMotion::Running, BEAT_MILLIS, false)
        );
    }

    #[test]
    fn the_glyph_shows_the_target_state_during_a_flourish() {
        assert_eq!(MascotMotion::Starting.glyph(), HeroGlyph::Pause);
        assert_eq!(MascotMotion::Stopping.glyph(), HeroGlyph::Play);
    }
}
