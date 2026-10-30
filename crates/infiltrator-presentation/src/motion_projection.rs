//! Motion and transition presentation projections (FLPARITY-017).
//!
//! Exposes unified transition projections for page routing, modal dialogs,
//! and sliding drawers, driving all surfaces with identical Material 3 curves
//! and durations without duplicating physics calculations in UI layers.

use crate::ui_presence::PresenceState;
use crate::ui_presence::UiPresence;
use infiltrator_contract::motion_spec::MotionPreference;
use infiltrator_contract::motion_spec::TransitionDirection;
use infiltrator_contract::motion_spec::TransitionPattern;
use infiltrator_contract::motion_spec::TransitionProgress;
use infiltrator_contract::motion_spec::TransitionSpec;
use std::env;

/// Projection for route page transitions across surfaces.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RouteTransitionProjection {
    /// The active transition pattern.
    pub pattern: TransitionPattern,
    /// Normalized animation progress 0.0..=1.0.
    pub progress: f32,
    /// Content opacity 0.0..=1.0.
    pub opacity: f32,
    /// Content scaling factor (e.g. 0.94..=1.0).
    pub scale: f32,
    /// Content horizontal translation in logical pixels.
    pub offset_x_px: f32,
    /// Content vertical translation in logical pixels.
    pub offset_y_px: f32,
    /// Whether the transition is currently animating.
    pub is_animating: bool,
}

impl Default for RouteTransitionProjection {
    fn default() -> Self {
        Self {
            pattern: TransitionPattern::FadeThrough,
            progress: 1.0,
            opacity: 1.0,
            scale: 1.0,
            offset_x_px: 0.0,
            offset_y_px: 0.0,
            is_animating: false,
        }
    }
}

/// Projection for modal dialogs and sliding drawer presentations.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModalTransitionProjection {
    /// The active transition pattern (e.g. ModalElevation, DrawerSlide).
    pub pattern: TransitionPattern,
    /// Lifecycle presence phase.
    pub presence_phase: UiPresence,
    /// Normalized progress 0.0..=1.0.
    pub progress: f32,
    /// Foreground surface opacity.
    pub opacity: f32,
    /// Foreground surface scale.
    pub scale: f32,
    /// Horizontal translation in logical pixels.
    pub offset_x_px: f32,
    /// Vertical translation in logical pixels.
    pub offset_y_px: f32,
    /// Background backdrop scrim opacity 0.0..=1.0.
    pub scrim_opacity: f32,
    /// Whether the modal currently accepts user interaction.
    pub is_interactive: bool,
    /// Whether an entrance or exit transition is currently active.
    pub is_animating: bool,
}

impl Default for ModalTransitionProjection {
    fn default() -> Self {
        Self {
            pattern: TransitionPattern::ModalElevation,
            presence_phase: UiPresence::Absent,
            progress: 0.0,
            opacity: 0.0,
            scale: 1.0,
            offset_x_px: 0.0,
            offset_y_px: 0.0,
            scrim_opacity: 0.0,
            is_interactive: false,
            is_animating: false,
        }
    }
}

/// Check if running under headless capture or screenshot testing mode.
pub fn is_headless_capture_active() -> bool {
    env::var("INFILTRATOR_SCREENSHOT_MODE")
        .is_ok_and(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        || env::var("CI_CAPTURE").is_ok_and(|v| v == "1" || v.eq_ignore_ascii_case("true"))
}

/// Project route page transition into visual parameters.
pub fn project_route_transition(
    pattern: TransitionPattern,
    direction: TransitionDirection,
    elapsed_ms: u32,
    preference: MotionPreference,
) -> RouteTransitionProjection {
    let effective_pref = if is_headless_capture_active() {
        MotionPreference::None
    } else {
        preference
    };

    let spec = TransitionSpec::new(pattern, direction, effective_pref);
    let sample = spec.interpolate(elapsed_ms);

    RouteTransitionProjection {
        pattern,
        progress: sample.progress,
        opacity: sample.opacity,
        scale: sample.scale,
        offset_x_px: sample.offset_x_px,
        offset_y_px: sample.offset_y_px,
        is_animating: !sample.is_completed,
    }
}

/// Project modal dialog or drawer transition based on presence state.
pub fn project_modal_transition(
    pattern: TransitionPattern,
    presence: &PresenceState,
    elapsed_ms: u32,
    preference: MotionPreference,
) -> ModalTransitionProjection {
    let phase = presence.phase();
    if phase == UiPresence::Absent {
        return ModalTransitionProjection {
            pattern,
            presence_phase: UiPresence::Absent,
            progress: 0.0,
            opacity: 0.0,
            scale: 1.0,
            offset_x_px: 0.0,
            offset_y_px: 0.0,
            scrim_opacity: 0.0,
            is_interactive: false,
            is_animating: false,
        };
    }

    if phase == UiPresence::Visible {
        let max_scrim = match pattern {
            TransitionPattern::DrawerSlide => 0.5,
            TransitionPattern::ModalElevation => 0.6,
            _ => 0.0,
        };
        return ModalTransitionProjection {
            pattern,
            presence_phase: UiPresence::Visible,
            progress: 1.0,
            opacity: 1.0,
            scale: 1.0,
            offset_x_px: 0.0,
            offset_y_px: 0.0,
            scrim_opacity: max_scrim,
            is_interactive: true,
            is_animating: false,
        };
    }

    let effective_pref = if is_headless_capture_active() {
        MotionPreference::None
    } else {
        preference
    };

    let direction = if phase == UiPresence::Entering {
        TransitionDirection::Enter
    } else {
        TransitionDirection::Exit
    };

    let spec = TransitionSpec::new(pattern, direction, effective_pref);
    let sample: TransitionProgress = spec.interpolate(elapsed_ms);

    ModalTransitionProjection {
        pattern,
        presence_phase: phase,
        progress: sample.progress,
        opacity: sample.opacity,
        scale: sample.scale,
        offset_x_px: sample.offset_x_px,
        offset_y_px: sample.offset_y_px,
        scrim_opacity: sample.scrim_opacity,
        is_interactive: false,
        is_animating: !sample.is_completed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_projections() {
        let route = RouteTransitionProjection::default();
        assert_eq!(route.progress, 1.0);
        assert_eq!(route.opacity, 1.0);
        assert!(!route.is_animating);

        let modal = ModalTransitionProjection::default();
        assert_eq!(modal.presence_phase, UiPresence::Absent);
        assert!(!modal.is_interactive);
        assert!(!modal.is_animating);
    }

    #[test]
    fn test_project_route_transition_flow() {
        let entering = project_route_transition(
            TransitionPattern::FadeThrough,
            TransitionDirection::Enter,
            125,
            MotionPreference::Full,
        );
        assert!(entering.is_animating);
        assert!(entering.opacity > 0.0 && entering.opacity < 1.0);
        assert!(entering.scale >= 0.94);

        let completed = project_route_transition(
            TransitionPattern::FadeThrough,
            TransitionDirection::Enter,
            300,
            MotionPreference::Full,
        );
        assert!(!completed.is_animating);
        assert_eq!(completed.opacity, 1.0);
        assert_eq!(completed.scale, 1.0);
    }

    #[test]
    fn test_project_modal_transition_presence() {
        let mut presence = PresenceState::new();
        let absent = project_modal_transition(
            TransitionPattern::ModalElevation,
            &presence,
            0,
            MotionPreference::Full,
        );
        assert_eq!(absent.presence_phase, UiPresence::Absent);
        assert!(!absent.is_interactive);

        presence.open();
        let entering = project_modal_transition(
            TransitionPattern::ModalElevation,
            &presence,
            100,
            MotionPreference::Full,
        );
        assert_eq!(entering.presence_phase, UiPresence::Entering);
        assert!(entering.is_animating);
        assert!(!entering.is_interactive);
        assert!(entering.scrim_opacity > 0.0);

        presence.advance(1.0);
        let visible = project_modal_transition(
            TransitionPattern::ModalElevation,
            &presence,
            0,
            MotionPreference::Full,
        );
        assert_eq!(visible.presence_phase, UiPresence::Visible);
        assert!(visible.is_interactive);
        assert!(!visible.is_animating);
        assert_eq!(visible.scrim_opacity, 0.6);
    }
}
