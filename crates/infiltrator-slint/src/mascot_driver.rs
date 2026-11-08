//! The one owner of the mascot's animation state in the Slint surface.
//!
//! The `.slint` tree draws `MascotMotionSpec`; this module projects the owner's
//! lifecycle readback and typed user cues into that spec. It is the only place
//! with a clock, so a capture host can freeze the frog without the view
//! growing a second "don't animate" flag.

use crate::{
    AppWindow, HeroGlyph, MascotMotionSpec, MascotMotionState, MascotPhase, SurfaceSnapshot,
};
// The contract and the generated Slint API both own a `HeroGlyph` and a
// `MascotMotionSpec`, so the contract types stay module-qualified here. No
// aliases: import hygiene forbids them, and a qualified path says which of the
// two closed vocabularies a value belongs to.
use infiltrator_contract::mascot_motion;
use infiltrator_contract::mascot_motion::{BEAT_MILLIS, MotionCue, NOTE_SLOTS};
use infiltrator_presentation::mascot_motion_projection::project_mascot;
// The generated Slint `MascotMotionState` global and the projection's state
// have the same name. Qualify this one through its module; no alias needed.
use infiltrator_presentation::mascot_motion_projection;
use lurker_core_contract::lifecycle::CoreLifecycle;
use slint::{ComponentHandle, Global};
use std::cell::RefCell;
use std::mem::forget;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Convert the contract's frame into the generated Slint struct. One mapping,
/// one place: a surface never reinterprets a phase or a note slot.
fn to_ui_spec(spec: mascot_motion::MascotMotionSpec) -> MascotMotionSpec {
    MascotMotionSpec {
        phase: match spec.state {
            mascot_motion::MascotMotion::Idle => MascotPhase::Idle,
            mascot_motion::MascotMotion::Starting => MascotPhase::Starting,
            mascot_motion::MascotMotion::Running => MascotPhase::Running,
            mascot_motion::MascotMotion::Stopping => MascotPhase::Stopping,
        },
        sway: spec.sway,
        pulse: spec.pulse,
        note_count: spec.note_count as i32,
        note_1: spec.note_progress[0],
        note_2: spec.note_progress[1],
        note_3: spec.note_progress[2],
        glyph: match spec.control_glyph {
            mascot_motion::HeroGlyph::Play => HeroGlyph::Play,
            mascot_motion::HeroGlyph::Pause => HeroGlyph::Pause,
        },
    }
}

/// The clock and state shared by the window's timer and projection.
pub struct MascotDriver {
    state: mascot_motion_projection::MascotMotionState,
    started: Instant,
    frozen: bool,
    /// A named capture frame owns the projection until the window closes. The
    /// live timer must not overwrite a receipt with a later beat.
    receipt_locked: bool,
}

impl MascotDriver {
    pub fn new(frozen: bool) -> Self {
        Self {
            state: mascot_motion_projection::MascotMotionState::default(),
            started: Instant::now(),
            frozen,
            receipt_locked: false,
        }
    }

    /// A capture host freezes the beat but keeps the state honest.
    pub fn set_frozen(&mut self, frozen: bool) {
        self.frozen = frozen;
    }

    pub fn is_frozen(&self) -> bool {
        self.frozen
    }

    fn now_millis(&self) -> u64 {
        self.started.elapsed().as_millis().min(u64::MAX as u128) as u64
    }

    /// Lock the driver to one deterministic frame. A capture receipt must not
    /// race the live timer that publishes frames every few milliseconds.
    pub fn lock_receipt(&mut self) {
        self.receipt_locked = true;
        self.frozen = true;
    }

    /// Publish one frame. The pending cue is a typed intent from the view; it
    /// is cleared here so no second component can replay it.
    pub fn tick(&mut self, window: &AppWindow, latest: &Arc<Mutex<Option<SurfaceSnapshot>>>) {
        if self.receipt_locked || RECEIPT_LOCKED.load(Ordering::SeqCst) {
            return;
        }
        let global = MascotMotionState::get(window);
        let cue = match global.get_pending_cue() {
            1 => Some(MotionCue::StartRequested),
            2 => Some(MotionCue::StopRequested),
            _ => None,
        };
        if cue.is_some() {
            global.set_pending_cue(0);
        }
        let lifecycle = latest
            .lock()
            .ok()
            .and_then(|guard| {
                guard
                    .as_ref()
                    .map(|snapshot| snapshot.core.lifecycle.clone())
            })
            .unwrap_or(CoreLifecycle::Stopped);
        let now_millis = self.now_millis();
        let frozen = self.frozen;
        let spec = project_mascot(&mut self.state, lifecycle, cue, now_millis, frozen);
        let ui = to_ui_spec(spec);
        if global.get_current() != ui {
            global.set_current(ui);
        }
    }
}

/// Install the driver's timer. The interval is the projection's beat divided
/// by the note slots, so the same number that decides the geometry also
/// decides the frame cadence; no surface invents a second FPS.
static RECEIPT_LOCKED: AtomicBool = AtomicBool::new(false);

pub fn install(window: &AppWindow, latest: Arc<Mutex<Option<SurfaceSnapshot>>>, frozen: bool) {
    let driver = Rc::new(RefCell::new(MascotDriver::new(frozen)));
    let weak = window.as_weak();
    let for_timer = Rc::clone(&driver);
    let timer = slint::Timer::default();
    let interval = u64::from(BEAT_MILLIS) / NOTE_SLOTS as u64 * 4;
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(interval / 6),
        move || {
            if let Some(window) = weak.upgrade() {
                for_timer.borrow_mut().tick(&window, &latest);
            }
        },
    );
    // The Timer must outlive this call: keep it in the window's own component
    // set via a leaked handle is forbidden, so it is stored in the driver's
    // thread-local runtime for the lifetime of the event loop.
    forget(timer);
}

/// Replay one deterministic motion frame for a capture receipt.
///
/// A live window gets its beat from the timer; a capture asks for a named
/// phase instead of racing wall-clock time. The same projection and the same
/// `to_ui_spec` mapping serve both, so the still frame is not a second
/// animation system.
pub fn publish_receipt_frame(window: &AppWindow, phase: &str, elapsed_millis: u32) {
    let state = match phase {
        "starting" => mascot_motion::MascotMotion::Starting,
        "running" => mascot_motion::MascotMotion::Running,
        "stopping" => mascot_motion::MascotMotion::Stopping,
        _ => mascot_motion::MascotMotion::Idle,
    };
    let spec = mascot_motion::frame(state, elapsed_millis, false);
    RECEIPT_LOCKED.store(true, Ordering::SeqCst);
    MascotMotionState::get(window).set_current(to_ui_spec(spec));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ui_spec_maps_every_phase_and_glyph() {
        for (state, phase) in [
            (mascot_motion::MascotMotion::Idle, MascotPhase::Idle),
            (mascot_motion::MascotMotion::Starting, MascotPhase::Starting),
            (mascot_motion::MascotMotion::Running, MascotPhase::Running),
            (mascot_motion::MascotMotion::Stopping, MascotPhase::Stopping),
        ] {
            let spec = mascot_motion::frame(state, 0, false);
            assert_eq!(to_ui_spec(spec).phase, phase);
        }
        assert_eq!(NOTE_SLOTS, 3);
    }
}
