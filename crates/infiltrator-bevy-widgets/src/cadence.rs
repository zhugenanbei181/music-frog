//! Energy-efficient rendering modes and frame pacing cadence control.

use bevy::ecs::event::Event;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::ResMut;

/// Target rendering cadence for power-saving / high-refresh modes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FramePacingMode {
    /// High refresh rate (60Hz - 144Hz) for smooth interactive animations.
    HighRefresh,
    /// Energy-efficient mode (10Hz - 30Hz) for static page viewing.
    #[default]
    PowerSaver,
    /// Background or minimized state (1Hz throttle).
    BackgroundThrottled,
    /// Suspended / fully asleep (0Hz, wake on OS/network event only).
    Suspended,
}

impl FramePacingMode {
    pub fn target_frame_time_ms(&self) -> u64 {
        match self {
            FramePacingMode::HighRefresh => 16,
            FramePacingMode::PowerSaver => 100,
            FramePacingMode::BackgroundThrottled => 1000,
            FramePacingMode::Suspended => 0,
        }
    }

    pub fn is_active(&self) -> bool {
        matches!(
            self,
            FramePacingMode::HighRefresh | FramePacingMode::PowerSaver
        )
    }
}

/// Dynamic cadence governor tracking active animation requests and idle decay.
#[derive(Resource, Clone, Debug, Default)]
pub struct CadenceGovernor {
    pub current_mode: FramePacingMode,
    pub active_high_refresh_frames: u32,
    pub is_window_focused: bool,
    pub is_window_visible: bool,
    pub is_low_power_device: bool,
}

impl CadenceGovernor {
    pub fn new() -> Self {
        Self {
            current_mode: FramePacingMode::PowerSaver,
            active_high_refresh_frames: 0,
            is_window_focused: true,
            is_window_visible: true,
            is_low_power_device: false,
        }
    }

    /// Request high refresh rate for the next `frames` render passes (e.g. for drag/scroll/animation).
    pub fn request_high_refresh(&mut self, frames: u32) {
        self.active_high_refresh_frames = self.active_high_refresh_frames.max(frames);
        self.recompute_mode();
    }

    /// Update window focus / visibility state from OS events.
    pub fn update_window_state(&mut self, focused: bool, visible: bool) {
        self.is_window_focused = focused;
        self.is_window_visible = visible;
        self.recompute_mode();
    }

    /// Tick one frame forward, decaying high-refresh counter.
    pub fn tick(&mut self) {
        if self.active_high_refresh_frames > 0 {
            self.active_high_refresh_frames -= 1;
        }
        self.recompute_mode();
    }

    fn recompute_mode(&mut self) {
        if !self.is_window_visible || !self.is_window_focused {
            self.current_mode = FramePacingMode::BackgroundThrottled;
        } else if self.active_high_refresh_frames > 0 && !self.is_low_power_device {
            self.current_mode = FramePacingMode::HighRefresh;
        } else {
            self.current_mode = FramePacingMode::PowerSaver;
        }
    }
}

/// Event requesting high-refresh wake-up for a duration in frames.
#[derive(Event, Clone, Copy, Debug, PartialEq, Eq)]
pub struct RequestCadenceWake(pub u32);

/// System to advance cadence governor decay each frame.
pub fn sync_cadence_governor(mut governor: ResMut<CadenceGovernor>) {
    governor.tick();
}

/// Rolling frame timing probe recording microsecond frame latencies and stutter events.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct FrameTimingProbe {
    pub rolling_times_ms: Vec<f32>,
    pub max_samples: usize,
    pub dropped_frames: usize,
    pub stutter_threshold_ms: f32,
}

impl Default for FrameTimingProbe {
    fn default() -> Self {
        Self {
            rolling_times_ms: Vec::with_capacity(60),
            max_samples: 60,
            dropped_frames: 0,
            stutter_threshold_ms: 16.67, // 60 Hz budget threshold
        }
    }
}

impl FrameTimingProbe {
    pub fn new(max_samples: usize, budget_ms: f32) -> Self {
        Self {
            rolling_times_ms: Vec::with_capacity(max_samples),
            max_samples: max_samples.max(1),
            dropped_frames: 0,
            stutter_threshold_ms: budget_ms,
        }
    }

    /// Record a completed render frame's execution time in milliseconds.
    pub fn record_frame(&mut self, frame_time_ms: f32) {
        if self.rolling_times_ms.len() >= self.max_samples {
            self.rolling_times_ms.remove(0);
        }
        self.rolling_times_ms.push(frame_time_ms);

        if frame_time_ms > self.stutter_threshold_ms {
            self.dropped_frames += 1;
        }
    }

    /// Average frames per second over the rolling sample window.
    pub fn average_fps(&self) -> f32 {
        if self.rolling_times_ms.is_empty() {
            return 60.0;
        }
        let mean_ms: f32 =
            self.rolling_times_ms.iter().sum::<f32>() / (self.rolling_times_ms.len() as f32);
        if mean_ms <= 1e-4 {
            1000.0
        } else {
            (1000.0 / mean_ms).max(1.0)
        }
    }

    /// Whether recent frames stayed strictly within frame budget with zero drops.
    pub fn is_stutter_free(&self) -> bool {
        self.dropped_frames == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_timing_probe_and_stutter_detection() {
        let mut probe = FrameTimingProbe::new(10, 16.67);
        assert!(probe.is_stutter_free());

        // Record 5 fast 8ms frames (120fps)
        for _ in 0..5 {
            probe.record_frame(8.0);
        }
        assert!(probe.is_stutter_free());
        assert!((probe.average_fps() - 125.0).abs() < 1.0);

        // Record 1 laggy 25ms frame -> drops frame
        probe.record_frame(25.0);
        assert!(!probe.is_stutter_free());
        assert_eq!(probe.dropped_frames, 1);
    }
}
