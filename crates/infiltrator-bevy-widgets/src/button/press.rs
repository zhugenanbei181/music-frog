//! The button's elastic press feedback: the typed phase machine that owns
//! the shrink on press and the spring back on release (EFFECTS-015 #54).
//!
//! The feedback is a **state machine**, not a pair of flags: the phase says
//! what the button is doing (resting, pressed, springing back) and the
//! shared spring integrator carries the scale. The integrator is the shared
//! one (`infiltrator_shared::effect_math`, the product's fourth-order
//! Runge-Kutta spring), so this module owns no physics of its own.
//!
//! The scale is applied through [`UiTransform`], never through the node's
//! size: a scale that changed the node's box would relayout the row the
//! button sits in, and a press must never move the layout around it.

use bevy::ecs::component::Component;
use bevy::ecs::query::Has;
use bevy::ecs::system::{Query, Res};
use bevy::math::Vec2;
use bevy::time::Time;
use bevy::ui::{Pressed, UiTransform};
use infiltrator_shared::effect_math::{SpringState, spring_settled, spring_step};

/// The scale a resting button sits at.
pub const BASE_SCALE: f32 = 1.0;

/// The scale a pressed button shrinks to.
pub const PRESSED_SCALE: f32 = 0.98;

/// The rebound window: the time a released button needs to be back at rest.
pub const REBOUND_DURATION_SECS: f32 = 0.08;

/// The spring stiffness of the shrink on press.
///
/// The shrink is the faster of the two motions: a press must answer the
/// pointer within a frame, so its spring is an order of magnitude stiffer
/// than the rebound's.
const PRESS_STIFFNESS: f32 = 6000.0;

/// The damping of the shrink on press. Just under critical, so the shrink
/// settles without a wobble.
const PRESS_DAMPING: f32 = 116.0;

/// The spring stiffness of the rebound.
const REBOUND_STIFFNESS: f32 = 1500.0;

/// The damping of the rebound. Under critical, so the button carries a
/// single small overshoot on its way back — the elastic part of the
/// feedback.
const REBOUND_DAMPING: f32 = 54.0;

/// The scale difference the spring counts as settled: a hundredth of a
/// percent of the button's own size, far below one pixel.
const SETTLE_EPSILON: f32 = 1.0e-4;

/// The phase of a button's press feedback.
///
/// The phase is the *only* record of what the button is doing: the scale
/// follows from it, and the phase is reached through the typed transitions
/// below, never by setting a flag beside another flag.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ButtonPressPhase {
    /// The pointer is not down and the button rests at its base scale.
    #[default]
    Resting,
    /// The pointer is down and the button shrinks toward the pressed ratio.
    Pressed,
    /// The pointer was released and the button springs back toward rest.
    Rebounding,
}

impl ButtonPressPhase {
    /// The phase a press moves the button into.
    pub fn on_press(self) -> Self {
        match self {
            Self::Resting | Self::Pressed | Self::Rebounding => Self::Pressed,
        }
    }

    /// The phase a release moves the button into.
    pub fn on_release(self) -> Self {
        match self {
            Self::Resting => Self::Resting,
            Self::Pressed | Self::Rebounding => Self::Rebounding,
        }
    }

    /// The phase the button falls back to once its rebound window elapsed.
    pub fn on_rebound_elapsed(self) -> Self {
        match self {
            Self::Resting => Self::Resting,
            Self::Pressed => Self::Pressed,
            Self::Rebounding => Self::Resting,
        }
    }

    /// The scale the phase drives the button toward.
    pub fn target_scale(self) -> f32 {
        match self {
            Self::Resting => BASE_SCALE,
            Self::Pressed => PRESSED_SCALE,
            Self::Rebounding => BASE_SCALE,
        }
    }

    /// The spring the phase integrates with: the stiffness and the damping.
    pub fn spring_params(self) -> (f32, f32) {
        match self {
            Self::Resting | Self::Rebounding => (REBOUND_STIFFNESS, REBOUND_DAMPING),
            Self::Pressed => (PRESS_STIFFNESS, PRESS_DAMPING),
        }
    }
}

/// The elastic press feedback of one button.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct ButtonPressScale {
    /// The phase the feedback is in.
    pub phase: ButtonPressPhase,
    /// The spring that carries the scale.
    pub spring: SpringState,
    /// The seconds spent in the current rebound.
    pub rebound_elapsed_secs: f32,
}

impl Default for ButtonPressScale {
    fn default() -> Self {
        Self {
            phase: ButtonPressPhase::Resting,
            spring: SpringState {
                value: BASE_SCALE,
                velocity: 0.0,
            },
            rebound_elapsed_secs: 0.0,
        }
    }
}

impl ButtonPressScale {
    /// The scale the button is drawn at right now.
    pub fn scale(&self) -> f32 {
        self.spring.value
    }

    /// Apply a press: the button starts shrinking.
    ///
    /// The transition is idempotent: a press that arrives while the button
    /// is already pressed changes nothing and does not restart the clock.
    pub fn press(&mut self) {
        let next = self.phase.on_press();
        if next != self.phase {
            self.phase = next;
            self.rebound_elapsed_secs = 0.0;
        }
    }

    /// Apply a release: the button starts springing back.
    ///
    /// The transition is idempotent for the same reason the press is: a
    /// release that arrives while the button is already springing back
    /// does not restart the rebound window.
    pub fn release(&mut self) {
        let next = self.phase.on_release();
        if next != self.phase {
            self.phase = next;
            self.rebound_elapsed_secs = 0.0;
        }
    }

    /// Advance the feedback by `dt` seconds.
    ///
    /// The spring inherits the interrupted motion's value and velocity, so
    /// a button that is pressed again mid-rebound continues from where it
    /// was instead of snapping.
    pub fn advance(&mut self, dt: f32) {
        let (stiffness, damping) = self.phase.spring_params();
        let target = self.phase.target_scale();
        self.spring = spring_step(self.spring, target, stiffness, damping, dt);
        if self.phase == ButtonPressPhase::Rebounding {
            self.rebound_elapsed_secs += dt;
            if self.rebound_elapsed_secs >= REBOUND_DURATION_SECS {
                self.phase = self.phase.on_rebound_elapsed();
            }
        }
    }

    /// Whether the spring has come to rest at the base scale.
    pub fn is_at_rest(&self) -> bool {
        spring_settled(self.spring, BASE_SCALE, SETTLE_EPSILON)
    }
}

/// Drive every button's elastic press feedback.
///
/// The press itself is the official `Pressed` marker the `bevy_ui_widgets`
/// button owns: this system only reads it and turns it into the typed
/// phase the spring integrates.
pub fn sync_button_press_scale(
    time: Res<Time>,
    mut buttons: Query<(&mut ButtonPressScale, Has<Pressed>, &mut UiTransform)>,
) {
    let dt = time.delta_secs();
    for (mut press, is_pressed, mut transform) in &mut buttons {
        if is_pressed {
            press.press();
        } else {
            press.release();
        }
        press.advance(dt);
        let scale = press.scale();
        let wanted = Vec2::splat(scale);
        if transform.scale != wanted {
            transform.scale = wanted;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One frame of the feedback at the given length.
    fn advance(press: &mut ButtonPressScale, frames: usize, dt: f32) {
        for _ in 0..frames {
            press.advance(dt);
        }
    }

    #[test]
    fn the_press_moves_the_button_into_the_pressed_phase() {
        let mut press = ButtonPressScale::default();
        assert_eq!(press.phase, ButtonPressPhase::Resting);
        press.press();
        assert_eq!(press.phase, ButtonPressPhase::Pressed);
        assert_eq!(press.phase.target_scale(), PRESSED_SCALE);
    }

    #[test]
    fn the_release_moves_the_button_into_the_rebound_phase() {
        let mut press = ButtonPressScale::default();
        press.press();
        press.release();
        assert_eq!(press.phase, ButtonPressPhase::Rebounding);
        assert_eq!(press.phase.target_scale(), BASE_SCALE);
    }

    #[test]
    fn a_repeated_press_does_not_restart_the_rebound_clock() {
        let mut press = ButtonPressScale::default();
        press.press();
        press.release();
        advance(&mut press, 2, 1.0 / 60.0);
        let elapsed = press.rebound_elapsed_secs;
        press.release();
        assert_eq!(press.rebound_elapsed_secs, elapsed);
    }

    #[test]
    fn the_pressed_button_shrinks_towards_the_pressed_ratio() {
        let mut press = ButtonPressScale::default();
        press.press();
        advance(&mut press, 6, 1.0 / 60.0);
        assert!(
            press.scale() < BASE_SCALE && press.scale() > PRESSED_SCALE - 0.001,
            "the shrink lands on the pressed ratio, got {}",
            press.scale()
        );
    }

    #[test]
    fn the_rebound_is_back_at_rest_within_the_rebound_window() {
        let mut press = ButtonPressScale::default();
        press.press();
        advance(&mut press, 6, 1.0 / 60.0);
        press.release();
        advance(&mut press, 5, 1.0 / 60.0);
        assert!(
            (press.scale() - BASE_SCALE).abs() < 0.001,
            "the button is back at rest 80ms after the release, got {}",
            press.scale()
        );
    }

    #[test]
    fn the_rebound_carries_one_small_elastic_overshoot() {
        let mut press = ButtonPressScale::default();
        press.press();
        advance(&mut press, 6, 1.0 / 60.0);
        press.release();
        let mut peak = press.scale();
        for _ in 0..12 {
            advance(&mut press, 1, 1.0 / 60.0);
            peak = peak.max(press.scale());
        }
        assert!(
            peak > BASE_SCALE,
            "the rebound overshoots the base scale, peak {peak}"
        );
        assert!(
            peak - BASE_SCALE < 0.01,
            "the overshoot stays a hair's breadth, peak {peak}"
        );
    }

    #[test]
    fn the_rebound_window_ends_the_rebound_phase() {
        let mut press = ButtonPressScale::default();
        press.press();
        press.release();
        advance(&mut press, 6, 1.0 / 60.0);
        assert_eq!(press.phase, ButtonPressPhase::Resting);
    }
}
