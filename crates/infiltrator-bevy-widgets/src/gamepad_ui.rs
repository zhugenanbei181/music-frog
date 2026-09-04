//! 10-Foot TV UI spatial navigation and gamepad analog stick smooth scrolling physics.

use bevy::ecs::resource::Resource;
use bevy::math::Vec2;

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

/// Gamepad analog stick scrolling accumulator.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct GamepadScrollState {
    pub left_stick: Vec2,
    pub right_stick: Vec2,
    pub scroll_velocity: Vec2,
    pub deadzone: f32,
    pub speed_multiplier: f32,
}

impl GamepadScrollState {
    pub fn new() -> Self {
        Self {
            left_stick: Vec2::ZERO,
            right_stick: Vec2::ZERO,
            scroll_velocity: Vec2::ZERO,
            deadzone: 0.15,
            speed_multiplier: 1200.0,
        }
    }

    pub fn update_sticks(&mut self, left: Vec2, right: Vec2) {
        self.left_stick = if left.length() > self.deadzone {
            left
        } else {
            Vec2::ZERO
        };
        self.right_stick = if right.length() > self.deadzone {
            right
        } else {
            Vec2::ZERO
        };
    }

    /// Step the scrolling physics forward by dt seconds, returning delta pixels to scroll.
    pub fn step(&mut self, dt: f32) -> Vec2 {
        let input = self.right_stick;
        let target_velocity = input * self.speed_multiplier;
        self.scroll_velocity = self
            .scroll_velocity
            .lerp(target_velocity, (dt * 15.0).min(1.0));
        self.scroll_velocity * dt
    }
}
