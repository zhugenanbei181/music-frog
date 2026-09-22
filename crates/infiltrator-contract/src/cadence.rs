//! Shared render cadence policy (DUAL-15-08).
//!
//! Both shells must detune to a low-power frame rate while the window is in
//! the background and return to full rate in the foreground. The *policy* is
//! product-wide and lives here; each surface projects it onto its own host
//! knob: Iced drives its animation frame-tick interval, Bevy configures its
//! winit update mode.
//!
//! The product rule (master plan DUAL-15-08): 60 FPS while active, 2 FPS while
//! backgrounded, and a full stop when the host reports a hard suspend
//! (occluded/invisible on hosts that expose that fact).

use std::time::Duration;

/// Which frame cadence the current window state calls for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RenderCadence {
    /// Foregrounded and visible: the full 60 FPS.
    Active,
    /// Backgrounded (unfocused but visible): the 2 FPS power-saver rate.
    Background,
    /// Occluded/invisible: no scheduled frames at all.
    Suspended,
}

impl RenderCadence {
    /// 60 FPS: the active cadence.
    pub const ACTIVE_FRAME_TIME_MS: u64 = 16;
    /// 2 FPS: the background cadence.
    pub const BACKGROUND_FRAME_TIME_MS: u64 = 500;
    /// Suspended: no scheduled frame.
    pub const SUSPENDED_FRAME_TIME_MS: u64 = 0;

    /// The cadence for a host that can only report focus (no occlusion fact).
    pub const fn from_focused(focused: bool) -> Self {
        if focused {
            Self::Active
        } else {
            Self::Background
        }
    }

    /// The cadence for a host that reports visibility and focus.
    pub const fn from_visible_focused(visible: bool, focused: bool) -> Self {
        if !visible {
            Self::Suspended
        } else if focused {
            Self::Active
        } else {
            Self::Background
        }
    }

    /// The milliseconds between scheduled frames.
    pub const fn frame_time_ms(self) -> u64 {
        match self {
            Self::Active => Self::ACTIVE_FRAME_TIME_MS,
            Self::Background => Self::BACKGROUND_FRAME_TIME_MS,
            Self::Suspended => Self::SUSPENDED_FRAME_TIME_MS,
        }
    }

    /// The tick interval, or `None` when the cadence schedules no frames.
    pub fn frame_interval(self) -> Option<Duration> {
        match self.frame_time_ms() {
            0 => None,
            millis => Some(Duration::from_millis(millis)),
        }
    }

    /// Whether this cadence schedules any frame at all.
    pub const fn is_animating(self) -> bool {
        !matches!(self, Self::Suspended)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_product_rates_are_sixty_and_two_fps() {
        assert_eq!(RenderCadence::Active.frame_time_ms(), 16);
        assert_eq!(RenderCadence::Background.frame_time_ms(), 500);
        assert!(
            RenderCadence::Active.frame_time_ms() <= 17,
            "the active cadence is ~60 FPS"
        );
        assert_eq!(1000 / RenderCadence::Background.frame_time_ms(), 2);
        assert!(RenderCadence::Background.frame_time_ms() > RenderCadence::Active.frame_time_ms());
    }

    #[test]
    fn every_host_fact_maps_to_one_cadence() {
        assert_eq!(RenderCadence::from_focused(true), RenderCadence::Active);
        assert_eq!(
            RenderCadence::from_focused(false),
            RenderCadence::Background
        );
        assert_eq!(
            RenderCadence::from_visible_focused(true, true),
            RenderCadence::Active
        );
        assert_eq!(
            RenderCadence::from_visible_focused(true, false),
            RenderCadence::Background
        );
        assert_eq!(
            RenderCadence::from_visible_focused(false, true),
            RenderCadence::Suspended
        );
        assert_eq!(RenderCadence::Suspended.frame_interval(), None);
        assert!(!RenderCadence::Suspended.is_animating());
        assert_eq!(
            RenderCadence::Background.frame_interval(),
            Some(Duration::from_millis(500))
        );
    }
}
