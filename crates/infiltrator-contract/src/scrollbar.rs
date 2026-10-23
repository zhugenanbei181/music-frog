//! Canonical scrollbar metrics and interaction contract across surfaces.
//!
//! Aligns scrollbar rendering and physics across Bevy, Iced, and Slint surfaces
//! by establishing one authoritative standard modeled after premier desktop
//! environments (macOS, Windows 11 Fluent, Material 3):
//!
//! 1. **Geometry**: Modern slim width (6px), pill corner radius (3px), 2px
//!    inner thumb margin, and a 16px safety gutter to prevent content overlay.
//! 2. **Interaction & Motion**: Auto-hiding floating state with 150ms smooth
//!    opacity transitions, resting at 0% and elevating to 40% ink opacity on
//!    hover or scroll events.
//! 3. **Scale Invariance**: Proportional rem/DPI scaling preserving minimum
//!    actionable thumb length (24px).

use crate::design_tokens::space;

/// Canonical dimensions and visual timing for scrollbars.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollbarMetrics {
    /// Track / thumb width in logical pixels (6.0px).
    pub track_width_px: f32,
    /// Pill corner radius in logical pixels (3.0px = half of width).
    pub thumb_radius_px: f32,
    /// Minimum thumb length (24.0px) preventing disappearance on massive lists.
    pub min_thumb_length_px: f32,
    /// Margin between thumb and track edge (2.0px).
    pub thumb_margin_px: f32,
    /// Content clearance padding / gutter preventing overlap (16.0px).
    pub content_gutter_px: f32,
    /// Transition animation duration for opacity changes in milliseconds (150ms).
    pub fade_duration_ms: u32,
    /// Resting opacity when idle and unhovered (0.0 = cleanly hidden).
    pub idle_opacity: f32,
    /// Active opacity when hovered, dragged, or scrolling (0.40 = soft ink wash).
    pub active_opacity: f32,
}

impl ScrollbarMetrics {
    /// Track width in logical pixels.
    pub const TRACK_WIDTH_PX: f32 = 6.0;
    /// Thumb corner radius in logical pixels.
    pub const THUMB_RADIUS_PX: f32 = 3.0;
    /// Minimum thumb length in logical pixels.
    pub const MIN_THUMB_LENGTH_PX: f32 = 24.0;
    /// Inner margin around thumb in logical pixels.
    pub const THUMB_MARGIN_PX: f32 = 2.0;
    /// Content safe gutter in logical pixels (aligned with 16pt design token).
    pub const CONTENT_GUTTER_PX: f32 = space::LG;
    /// Fade transition duration in milliseconds.
    pub const FADE_DURATION_MS: u32 = 150;
    /// Idle resting opacity.
    pub const IDLE_OPACITY: f32 = 0.0;
    /// Active interactive opacity.
    pub const ACTIVE_OPACITY: f32 = 0.40;

    /// The premier canonical scrollbar specification.
    pub const DEFAULT: Self = Self {
        track_width_px: Self::TRACK_WIDTH_PX,
        thumb_radius_px: Self::THUMB_RADIUS_PX,
        min_thumb_length_px: Self::MIN_THUMB_LENGTH_PX,
        thumb_margin_px: Self::THUMB_MARGIN_PX,
        content_gutter_px: Self::CONTENT_GUTTER_PX,
        fade_duration_ms: Self::FADE_DURATION_MS,
        idle_opacity: Self::IDLE_OPACITY,
        active_opacity: Self::ACTIVE_OPACITY,
    };

    /// Whether the geometry represents a true pill shape (radius is half width).
    pub const fn is_true_pill(&self) -> bool {
        (self.thumb_radius_px * 2.0) == self.track_width_px
    }

    /// Compute scaled minimum thumb length given a rem root size factor.
    pub fn scaled_min_thumb_length(&self, rem_factor: f32) -> f32 {
        self.min_thumb_length_px * rem_factor.max(0.5)
    }
}

/// Explicit visibility policy for scroll indicators.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ScrollbarVisibilityPolicy {
    /// Modern floating behavior: hidden at rest, fades in smoothly on interaction.
    #[default]
    AutoHide,
    /// Traditional persistent behavior: track and thumb stay constantly visible.
    AlwaysVisible,
    /// Completely hidden scrollbar (e.g. for touch drag-only strips).
    Hidden,
}

/// Typed mutual-exclusion state for scrollbar fade-in / fade-out animations.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ScrollbarPhase {
    /// Stationary resting state with zero or base opacity.
    Idle,
    /// Animating towards active opacity (progress in `0.0..=1.0`).
    FadingIn { progress: f32 },
    /// Fully visible and active.
    Active,
    /// Animating back to idle resting state (progress in `0.0..=1.0`).
    FadingOut { progress: f32 },
}

impl Default for ScrollbarPhase {
    fn default() -> Self {
        Self::Idle
    }
}

impl ScrollbarPhase {
    /// Current opacity resolved from metrics.
    pub fn current_opacity(&self, metrics: &ScrollbarMetrics) -> f32 {
        match *self {
            Self::Idle => metrics.idle_opacity,
            Self::Active => metrics.active_opacity,
            Self::FadingIn { progress } => {
                let p = progress.clamp(0.0, 1.0);
                metrics.idle_opacity + (metrics.active_opacity - metrics.idle_opacity) * p
            }
            Self::FadingOut { progress } => {
                let p = progress.clamp(0.0, 1.0);
                metrics.active_opacity - (metrics.active_opacity - metrics.idle_opacity) * p
            }
        }
    }

    /// Advance transition by elapsed time delta in seconds.
    pub fn advance(self, delta_secs: f32, metrics: &ScrollbarMetrics) -> Self {
        let duration_secs = (metrics.fade_duration_ms as f32 / 1000.0).max(0.001);
        let step = delta_secs / duration_secs;
        match self {
            Self::FadingIn { progress } => {
                let next = progress + step;
                if next >= 1.0 {
                    Self::Active
                } else {
                    Self::FadingIn { progress: next }
                }
            }
            Self::FadingOut { progress } => {
                let next = progress + step;
                if next >= 1.0 {
                    Self::Idle
                } else {
                    Self::FadingOut { progress: next }
                }
            }
            settled => settled,
        }
    }

    /// Trigger user interaction: start or reset fade-in.
    pub fn on_interaction(self) -> Self {
        match self {
            Self::Active => Self::Active,
            Self::Idle | Self::FadingOut { .. } => Self::FadingIn { progress: 0.0 },
            Self::FadingIn { progress } => Self::FadingIn { progress },
        }
    }

    /// Trigger interaction idle timeout: begin fade-out.
    pub fn on_idle_timeout(self) -> Self {
        match self {
            Self::Idle => Self::Idle,
            Self::Active | Self::FadingIn { .. } => Self::FadingOut { progress: 0.0 },
            Self::FadingOut { progress } => Self::FadingOut { progress },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canonical_metrics_adheres_to_pill_geometry() {
        let metrics = ScrollbarMetrics::DEFAULT;
        assert!(
            metrics.is_true_pill(),
            "thumb radius must form an exact pill cap"
        );
        assert_eq!(metrics.track_width_px, 6.0);
        assert_eq!(metrics.thumb_radius_px, 3.0);
        assert_eq!(metrics.content_gutter_px, 16.0);
        assert_eq!(metrics.fade_duration_ms, 150);
        assert_eq!(metrics.idle_opacity, 0.0);
        assert_eq!(metrics.active_opacity, 0.40);
    }

    #[test]
    fn test_scrollbar_phase_opacity_interpolation() {
        let metrics = ScrollbarMetrics::DEFAULT;
        assert_eq!(ScrollbarPhase::Idle.current_opacity(&metrics), 0.0);
        assert_eq!(ScrollbarPhase::Active.current_opacity(&metrics), 0.40);

        let half_in = ScrollbarPhase::FadingIn { progress: 0.5 };
        assert!((half_in.current_opacity(&metrics) - 0.20).abs() < 1e-5);

        let half_out = ScrollbarPhase::FadingOut { progress: 0.5 };
        assert!((half_out.current_opacity(&metrics) - 0.20).abs() < 1e-5);
    }

    #[test]
    fn test_scrollbar_phase_advance_settles() {
        let metrics = ScrollbarMetrics::DEFAULT;
        let phase = ScrollbarPhase::FadingIn { progress: 0.0 };
        // 150ms animation completed in a single 0.2s step
        let settled = phase.advance(0.2, &metrics);
        assert_eq!(settled, ScrollbarPhase::Active);

        let fading_out = ScrollbarPhase::FadingOut { progress: 0.0 };
        let idle = fading_out.advance(0.2, &metrics);
        assert_eq!(idle, ScrollbarPhase::Idle);
    }
}
