//! 10-Foot TV UI spatial navigation and gamepad analog stick smooth scrolling physics.

use crate::focus::{FocusDirection, find_spatial_neighbor};
use bevy::ecs::entity::Entity;
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

    pub fn focus_direction(self) -> FocusDirection {
        match self {
            Self::Up => FocusDirection::Up,
            Self::Down => FocusDirection::Down,
            Self::Left => FocusDirection::Left,
            Self::Right => FocusDirection::Right,
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

/// A focusable widget with its layout centre in TV space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GamepadFocusTarget {
    pub entity: Entity,
    pub center: Vec2,
}

/// D-pad spatial focus controller over the widget tree. Directional movement
/// reuses the shared spatial-neighbour owner in [`crate::focus`]; wrapping is
/// layered on only when the caller opts in.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct GamepadFocusController {
    pub targets: Vec<GamepadFocusTarget>,
    pub focused: Option<Entity>,
    pub wrap: bool,
    /// Perpendicular distance within which two widgets count as the same row
    /// or column during wrapping.
    pub row_tolerance: f32,
}

impl Default for GamepadFocusController {
    fn default() -> Self {
        Self {
            targets: Vec::new(),
            focused: None,
            wrap: true,
            row_tolerance: 8.0,
        }
    }
}

impl GamepadFocusController {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register (or update) a focusable widget. The first registered widget
    /// becomes the initial focus target.
    pub fn register(&mut self, entity: Entity, center: Vec2) {
        if let Some(target) = self
            .targets
            .iter_mut()
            .find(|target| target.entity == entity)
        {
            target.center = center;
        } else {
            self.targets.push(GamepadFocusTarget { entity, center });
        }
        if self.focused.is_none() {
            self.focused = Some(entity);
        }
    }

    pub fn focus_first(&mut self) -> Option<Entity> {
        self.focused = self.targets.first().map(|target| target.entity);
        self.focused
    }

    /// Focus a registered entity; returns `false` for an unknown target.
    pub fn set_focus(&mut self, entity: Entity) -> bool {
        if self.targets.iter().any(|target| target.entity == entity) {
            self.focused = Some(entity);
            true
        } else {
            false
        }
    }

    /// Handle a gamepad navigation action. Non-directional actions leave focus
    /// unchanged and return the current target.
    pub fn handle_action(&mut self, action: GamepadNavAction) -> Option<Entity> {
        match GamepadDirection::from_action(action) {
            Some(direction) => self.move_focus(direction),
            None => self.focused,
        }
    }

    /// Move focus one step in `direction`, wrapping when enabled.
    pub fn move_focus(&mut self, direction: GamepadDirection) -> Option<Entity> {
        let current = self.current_target()?;
        let pairs: Vec<(Entity, Vec2)> = self
            .targets
            .iter()
            .map(|target| (target.entity, target.center))
            .collect();
        let neighbour = find_spatial_neighbor(current.center, direction.focus_direction(), &pairs)
            .or_else(|| {
                self.wrap
                    .then(|| self.wrap_target(current, direction))
                    .flatten()
            });
        if let Some(entity) = neighbour
            && entity != current.entity
        {
            self.focused = Some(entity);
        }
        self.focused
    }

    fn current_target(&self) -> Option<GamepadFocusTarget> {
        let focused = self.focused?;
        self.targets
            .iter()
            .copied()
            .find(|target| target.entity == focused)
    }

    /// Wrap to the extreme widget in the direction of travel, preferring the
    /// same perpendicular band (row/column) as the current target.
    fn wrap_target(
        &self,
        current: GamepadFocusTarget,
        direction: GamepadDirection,
    ) -> Option<Entity> {
        let axis = direction.unit_axis();
        let perpendicular = Vec2::new(-axis.y, axis.x);
        let in_band = |target: &GamepadFocusTarget| {
            (target.center - current.center).dot(perpendicular).abs() <= self.row_tolerance
        };
        let extreme = |only_band: bool| -> Option<Entity> {
            self.targets
                .iter()
                .filter(|target| target.entity != current.entity && (!only_band || in_band(target)))
                .min_by(|left, right| left.center.dot(axis).total_cmp(&right.center.dot(axis)))
                .map(|target| target.entity)
        };
        extreme(true).or_else(|| extreme(false))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entity(raw: u32) -> Entity {
        Entity::from_raw_u32(raw).expect("valid entity id")
    }

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
    fn test_gamepad_focus_moves_spatially() {
        let mut controller = GamepadFocusController::new();
        let left = entity(1);
        let middle = entity(2);
        let right = entity(3);
        controller.register(left, Vec2::new(0.0, 0.0));
        controller.register(middle, Vec2::new(100.0, 0.0));
        controller.register(right, Vec2::new(200.0, 0.0));

        assert_eq!(controller.focused, Some(left));
        assert_eq!(
            controller.handle_action(GamepadNavAction::DpadRight),
            Some(middle)
        );
        assert_eq!(
            controller.handle_action(GamepadNavAction::DpadRight),
            Some(right)
        );
        // Non-directional actions leave focus unchanged.
        assert_eq!(
            controller.handle_action(GamepadNavAction::ButtonAConfirm),
            Some(right)
        );
        assert_eq!(
            controller.handle_action(GamepadNavAction::DpadLeft),
            Some(middle)
        );
    }

    #[test]
    fn test_gamepad_focus_wraps_and_can_be_disabled() {
        let mut controller = GamepadFocusController::new();
        let left = entity(1);
        let right = entity(2);
        controller.register(left, Vec2::new(0.0, 0.0));
        controller.register(right, Vec2::new(100.0, 0.0));

        assert!(controller.set_focus(right));
        assert_eq!(controller.move_focus(GamepadDirection::Right), Some(left));

        controller.wrap = false;
        assert!(controller.set_focus(right));
        assert_eq!(controller.move_focus(GamepadDirection::Right), Some(right));
        assert!(!controller.set_focus(entity(99)));
    }

    #[test]
    fn test_gamepad_focus_wrap_prefers_same_row() {
        let mut controller = GamepadFocusController::new();
        let top_left = entity(1);
        let top_right = entity(2);
        let bottom_left = entity(3);
        controller.register(top_left, Vec2::new(0.0, 0.0));
        controller.register(top_right, Vec2::new(100.0, 0.0));
        controller.register(bottom_left, Vec2::new(0.0, 50.0));

        assert!(controller.set_focus(top_right));
        // Wrap right -> same-row leftmost, not the bottom row.
        assert_eq!(
            controller.move_focus(GamepadDirection::Right),
            Some(top_left)
        );
    }
}
