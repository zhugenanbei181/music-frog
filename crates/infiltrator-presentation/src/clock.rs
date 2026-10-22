//! The presentation clock and the frame demand it publishes
//! (EFFECTS-011 #37, #38).
//!
//! The presentation layer renders against its own clock, not against the
//! owner's poll interval: the clock is measured in microseconds, so a frame
//! that arrives late is still timed correctly, and a surface that has
//! nothing to animate stops asking for frames instead of spinning at the
//! display rate.

use std::time::{Duration, Instant};

/// The frame interval the clock asks for while something is moving.
pub const ACTIVE_FRAME_INTERVAL: Duration = Duration::from_millis(16);

/// Whether the presentation needs another frame (EFFECTS-011 #38).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameDemand {
    /// Nothing is moving: the surface must stop asking for frames.
    Idle,
    /// Something is moving: the surface asks for a frame at the interval.
    Active,
}

/// The presentation's own smooth clock (EFFECTS-011 #37).
///
/// The clock is independent of the owner's poll: it measures the time since
/// the last frame in microseconds, so a slow frame is a long frame and not a
/// wrong one.
#[derive(Clone, Debug)]
pub struct RenderClock {
    origin: Instant,
    last: Instant,
    running: bool,
}

impl Default for RenderClock {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderClock {
    /// A clock that is running.
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            origin: now,
            last: now,
            running: true,
        }
    }

    /// The time since the clock started, in microseconds.
    pub fn elapsed_micros(&self) -> u64 {
        self.origin.elapsed().as_micros() as u64
    }

    /// The time since the last frame, in microseconds.
    pub fn delta_micros(&self) -> u64 {
        self.last.elapsed().as_micros() as u64
    }

    /// Whether the clock is running.
    pub const fn is_running(&self) -> bool {
        self.running
    }

    /// Start the clock again.
    pub fn start(&mut self) {
        self.running = true;
        self.last = Instant::now();
    }

    /// Stop the clock.
    ///
    /// A stopped clock publishes [`FrameDemand::Idle`], so the surface stops
    /// asking for frames and the device goes back to sleep.
    pub fn stop(&mut self) {
        self.running = false;
    }

    /// What the surface should do after this frame.
    pub const fn frame_demand(&self) -> FrameDemand {
        if self.running {
            FrameDemand::Active
        } else {
            FrameDemand::Idle
        }
    }

    /// Mark the end of a frame.
    pub fn tick(&mut self) {
        self.last = Instant::now();
    }

    /// Whether a frame is due.
    pub fn frame_due(&self, interval: Duration) -> bool {
        self.running && self.last.elapsed() >= interval
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn a_running_clock_demands_frames_and_a_stopped_one_does_not() {
        let mut clock = RenderClock::new();
        assert!(clock.is_running());
        assert_eq!(clock.frame_demand(), FrameDemand::Active);

        clock.stop();
        assert!(!clock.is_running());
        assert_eq!(
            clock.frame_demand(),
            FrameDemand::Idle,
            "an idle presentation must stop asking for frames"
        );
        assert!(!clock.frame_due(ACTIVE_FRAME_INTERVAL));

        clock.start();
        assert_eq!(clock.frame_demand(), FrameDemand::Active);
    }

    #[test]
    fn the_clock_measures_the_frame_and_the_elapsed_time() {
        let mut clock = RenderClock::new();
        thread::sleep(Duration::from_millis(2));
        let delta = clock.delta_micros();
        assert!(
            delta > 0,
            "the clock must measure the frame in microseconds"
        );
        clock.tick();
        assert!(
            clock.delta_micros() < delta + 100_000,
            "ticking the clock restarts the measurement"
        );
        assert!(clock.elapsed_micros() >= delta);
    }

    #[test]
    fn a_frame_is_due_after_the_interval() {
        let mut clock = RenderClock::new();
        clock.stop();
        thread::sleep(Duration::from_millis(2));
        assert!(!clock.frame_due(Duration::from_millis(1)));
        clock.start();
        assert!(clock.frame_due(Duration::from_millis(1)));
        clock.tick();
        assert!(!clock.frame_due(Duration::from_millis(10)));
    }
}
