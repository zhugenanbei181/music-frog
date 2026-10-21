//! 10-Foot TV UI input vocabulary and gamepad analog stick smooth scrolling physics.
//!
//! The focus movement itself is not owned here: the official
//! `bevy_ui::auto_directional_navigation` navigator owns the neighbour search
//! and moves the official `InputFocus`. This module owns the *input*
//! vocabulary the shell forwards — the D-pad and face-button actions, the
//! analog-stick direction with its radial deadzone, and the scroll physics.

use bevy::ecs::resource::Resource;
use bevy::math::{CompassOctant, Vec2};

/// Standard gamepad navigation action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GamepadNavAction {
    DpadUp,
    DpadDown,
    DpadLeft,
    DpadRight,
    ButtonAConfirm,
    ButtonBCancel,
    ButtonXQuickTest,
    ButtonYSearch,
    TriggerLeftPrevTab,
    TriggerRightNextTab,
}

/// Apply a radial deadzone and rescale the surviving magnitude so movement
/// ramps from `0` at the deadzone edge to full deflection at `1.0`.
pub fn radial_deadzone(stick: Vec2, deadzone: f32) -> Vec2 {
    let magnitude = stick.length();
    if magnitude <= deadzone || deadzone >= 1.0 {
        return Vec2::ZERO;
    }
    let scaled = ((magnitude - deadzone) / (1.0 - deadzone)).min(1.0);
    stick.normalize_or_zero() * scaled
}

/// Cardinal direction derived from a D-pad press or analog stick deflection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GamepadDirection {
    Up,
    Down,
    Left,
    Right,
}

impl GamepadDirection {
    pub fn from_action(action: GamepadNavAction) -> Option<Self> {
        match action {
            GamepadNavAction::DpadUp => Some(Self::Up),
            GamepadNavAction::DpadDown => Some(Self::Down),
            GamepadNavAction::DpadLeft => Some(Self::Left),
            GamepadNavAction::DpadRight => Some(Self::Right),
            _ => None,
        }
    }

    /// The official compass octant this direction asks the official
    /// directional navigation for.
    pub fn compass_octant(self) -> CompassOctant {
        match self {
            Self::Up => CompassOctant::North,
            Self::Down => CompassOctant::South,
            Self::Left => CompassOctant::West,
            Self::Right => CompassOctant::East,
        }
    }

    /// Unit vector along the direction of travel in UI space (y grows downward,
    /// matching Bevy UI layout coordinates).
    pub fn unit_axis(self) -> Vec2 {
        match self {
            Self::Up => Vec2::new(0.0, -1.0),
            Self::Down => Vec2::new(0.0, 1.0),
            Self::Left => Vec2::new(-1.0, 0.0),
            Self::Right => Vec2::new(1.0, 0.0),
        }
    }

    /// Dominant direction of an analog stick after radial deadzone filtering.
    pub fn from_stick(stick: Vec2, deadzone: f32) -> Option<Self> {
        let filtered = radial_deadzone(stick, deadzone);
        if filtered == Vec2::ZERO {
            return None;
        }
        if filtered.x.abs() >= filtered.y.abs() {
            Some(if filtered.x > 0.0 {
                Self::Right
            } else {
                Self::Left
            })
        } else {
            Some(if filtered.y > 0.0 {
                Self::Down
            } else {
                Self::Up
            })
        }
    }
}

/// Gamepad analog stick scrolling accumulator.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct GamepadScrollState {
    pub left_stick: Vec2,
    pub right_stick: Vec2,
    pub scroll_velocity: Vec2,
    pub deadzone: f32,
    pub speed_multiplier: f32,
    /// Exponential smoothing rate applied per second while approaching target
    /// velocity; higher values react faster.
    pub smoothing_rate: f32,
}

impl GamepadScrollState {
    pub fn new() -> Self {
        Self {
            left_stick: Vec2::ZERO,
            right_stick: Vec2::ZERO,
            scroll_velocity: Vec2::ZERO,
            deadzone: 0.15,
            speed_multiplier: 1200.0,
            smoothing_rate: 15.0,
        }
    }

    pub fn update_sticks(&mut self, left: Vec2, right: Vec2) {
        self.left_stick = radial_deadzone(left, self.deadzone);
        self.right_stick = radial_deadzone(right, self.deadzone);
    }

    /// Step the scrolling physics forward by dt seconds, returning delta pixels to scroll.
    pub fn step(&mut self, dt: f32) -> Vec2 {
        let input = self.right_stick;
        let target_velocity = input * self.speed_multiplier;
        self.scroll_velocity = self
            .scroll_velocity
            .lerp(target_velocity, (dt * self.smoothing_rate).min(1.0));
        self.scroll_velocity * dt
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_radial_deadzone_and_analog_direction() {
        assert_eq!(radial_deadzone(Vec2::new(0.05, 0.05), 0.15), Vec2::ZERO);
        assert_eq!(radial_deadzone(Vec2::new(0.0, 0.15), 0.15), Vec2::ZERO);
        assert_eq!(
            radial_deadzone(Vec2::new(0.0, 1.0), 0.15),
            Vec2::new(0.0, 1.0)
        );

        // Full deflection survives; a point just past the edge ramps from zero.
        let edge = radial_deadzone(Vec2::new(0.0, 0.25), 0.15);
        assert!(edge.y > 0.0 && edge.y < 0.25);

        assert_eq!(
            GamepadDirection::from_stick(Vec2::new(0.0, 0.05), 0.15),
            None
        );
        assert_eq!(
            GamepadDirection::from_stick(Vec2::new(0.9, 0.1), 0.15),
            Some(GamepadDirection::Right)
        );
        assert_eq!(
            GamepadDirection::from_stick(Vec2::new(-0.8, 0.0), 0.15),
            Some(GamepadDirection::Left)
        );
        assert_eq!(
            GamepadDirection::from_stick(Vec2::new(0.0, -0.8), 0.15),
            Some(GamepadDirection::Up)
        );
        assert_eq!(
            GamepadDirection::from_stick(Vec2::new(0.1, 0.9), 0.15),
            Some(GamepadDirection::Down)
        );
    }

    #[test]
    fn test_gamepad_scroll_smoothing_ramps_toward_target() {
        let mut state = GamepadScrollState::new();
        state.update_sticks(Vec2::ZERO, Vec2::new(0.0, 1.0));
        assert_eq!(state.right_stick, Vec2::new(0.0, 1.0));

        let first = state.step(0.016);
        let second = state.step(0.016);
        assert!(second.y > first.y);
        assert!(state.scroll_velocity.y > 0.0);

        // Releasing the stick decays velocity back toward zero.
        state.update_sticks(Vec2::ZERO, Vec2::ZERO);
        assert_eq!(state.right_stick, Vec2::ZERO);
        state.step(0.05);
        assert!(state.scroll_velocity.y < 1.0 * state.speed_multiplier);
    }

    #[test]
    fn test_the_dpad_speaks_the_official_compass_octants() {
        assert_eq!(GamepadDirection::Up.compass_octant(), CompassOctant::North);
        assert_eq!(
            GamepadDirection::Down.compass_octant(),
            CompassOctant::South
        );
        assert_eq!(GamepadDirection::Left.compass_octant(), CompassOctant::West);
        assert_eq!(
            GamepadDirection::Right.compass_octant(),
            CompassOctant::East
        );
    }
}
