//! Energy-efficient rendering modes and frame pacing cadence control.
//!
//! The three product cadences mirror the shared contract
//! (`infiltrator_contract::cadence::RenderCadence`, DUAL-15-08): 16ms active
//! (≈60 FPS), 500ms background (2 FPS), 0ms suspended. This crate is
//! business-agnostic by charter and cannot depend on contract, so the mirror
//! is enforced by `crates/infiltrator-bevy-ui/tests/headless/cadence_tests.rs`
//! (the retired numeric-scan guard is replaced by that headless mirror test).
//! [`FramePacingMode::PowerSaver`] is the widget layer's own intermediate
//! energy saver and has no product-cadence counterpart.

use bevy::ecs::event::Event;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::ResMut;

/// Target rendering cadence for power-saving / high-refresh modes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FramePacingMode {
    /// High refresh rate: the shared active cadence (16ms, ≈60 FPS).
    HighRefresh,
    /// Energy-efficient mode (10Hz - 30Hz) for static page viewing.
    #[default]
    PowerSaver,
    /// Background window: the shared 2 FPS throttle (500ms).
    BackgroundThrottled,
    /// Suspended / fully asleep: no scheduled frames.
    Suspended,
}

impl FramePacingMode {
    pub fn target_frame_time_ms(&self) -> u64 {
        match self {
            FramePacingMode::HighRefresh => 16,
            FramePacingMode::PowerSaver => 100,
            FramePacingMode::BackgroundThrottled => 500,
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
