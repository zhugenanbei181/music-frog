//! Shell-level D-pad / analog-stick focus navigation for the 10-foot TV surface
//! (BEVY-038).
//!
//! The widget layer owns the pure spatial controller
//! ([`GamepadFocusController`]) and its deadzone math; this module owns the
//! product wiring: a message seam a host adapter feeds, a system that measures
//! the mounted focusable controls into the controller, a typed shell focus state
//! so "nothing is focused" is explicit rather than an absent entity, and the
//! confirm/cancel mapping onto the shell's existing activation vocabulary.

use crate::route::NavigateBack;
use bevy::app::{App, Plugin, Update};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::message::{Message, MessageReader};
use bevy::ecs::query::{Has, Or, With};
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::math::Vec2;
use bevy::ui::{ComputedNode, UiGlobalTransform};
use bevy::ui_widgets::{Activate, Button};
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::focus::Focused;
use infiltrator_bevy_widgets::gamepad_ui::{
    GamepadDirection, GamepadFocusController, GamepadFocusTarget, GamepadNavAction,
};

/// Marker on every shell control reachable by D-pad / analog-stick focus.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ShellFocusable;

/// Shell focus targets: the explicit shell markers plus every mounted button, so
/// D-pad focus reaches page content without each page re-declaring the seam.
type ShellFocusTargetFilter = Or<(With<ShellFocusable>, With<Button>)>;

/// Default radial deadzone applied to analog-stick deflection.
pub const SHELL_STICK_DEADZONE: f32 = 0.15;

/// Shell-level focus configuration: the analog deadzone and whether directional
/// movement wraps at the edges.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct ShellFocusConfig {
    pub deadzone: f32,
    pub wrap: bool,
}

impl Default for ShellFocusConfig {
    fn default() -> Self {
        Self {
            deadzone: SHELL_STICK_DEADZONE,
            wrap: true,
        }
    }
}

/// Typed shell focus: an explicit "no focus" state distinct from a missing
/// entity, so a host or test can assert the shell has not focused anything.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ShellFocusState {
    #[default]
    NoFocus,
    Focused(Entity),
}

impl ShellFocusState {
    pub fn focused_entity(self) -> Option<Entity> {
        match self {
            Self::Focused(entity) => Some(entity),
            Self::NoFocus => None,
        }
    }
}

/// A gamepad navigation request. A host adapter forwards D-pad/face-button
/// actions and raw analog-stick samples; the shell never reads a device itself.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub enum ShellFocusNav {
    Action(GamepadNavAction),
    Stick(Vec2),
}

/// Rebuild the controller's targets from the measured layout. A focusable that
/// is disabled, hidden, or not yet laid out (no finite, positive measured box)
/// is not reachable, so it is skipped; when nothing is measured the controller
/// keeps the targets a host or test seeded.
pub fn sync_shell_focus_targets(
    mut controller: ResMut<GamepadFocusController>,
    config: Res<ShellFocusConfig>,
    focusables: Query<
        (
            Entity,
            &ComputedNode,
            &UiGlobalTransform,
            Option<&ButtonDisabled>,
        ),
        ShellFocusTargetFilter,
    >,
) {
    let measured: Vec<GamepadFocusTarget> = focusables
        .iter()
        .filter_map(|(entity, computed, transform, disabled)| {
            let size = computed.size();
            let reachable =
                size.is_finite() && size.x > 0.0 && size.y > 0.0 && !disabled.is_some_and(|d| d.0);
            reachable.then_some(GamepadFocusTarget {
                entity,
                center: transform.affine().translation,
            })
        })
        .collect();
    controller.wrap = config.wrap;
    if measured.is_empty() {
        return;
    }
    controller.targets = measured;
    let still_focused = controller
        .focused
        .is_some_and(|focused| controller.targets.iter().any(|t| t.entity == focused));
    if !still_focused {
        controller.focused = controller.targets.first().map(|target| target.entity);
    }
}

/// Handle each navigation request: directional actions and stick samples move
/// focus; confirm activates the focused control; cancel navigates back.
pub fn advance_shell_focus(
    mut navs: MessageReader<ShellFocusNav>,
    config: Res<ShellFocusConfig>,
    mut controller: ResMut<GamepadFocusController>,
    mut commands: Commands,
) {
    for nav in navs.read() {
        match *nav {
            ShellFocusNav::Action(GamepadNavAction::ButtonAConfirm) => {
                if let Some(entity) = controller.focused {
                    commands.trigger(Activate { entity });
                }
            }
            ShellFocusNav::Action(GamepadNavAction::ButtonBCancel) => {
                commands.trigger(NavigateBack);
            }
            ShellFocusNav::Action(action) => {
                controller.handle_action(action);
            }
            ShellFocusNav::Stick(stick) => {
                if let Some(direction) = GamepadDirection::from_stick(stick, config.deadzone) {
                    controller.move_focus(direction);
                }
            }
        }
    }
}

/// Publish the controller's focus as the typed shell focus state.
pub fn sync_shell_focus_state(
    controller: Res<GamepadFocusController>,
    mut state: ResMut<ShellFocusState>,
) {
    let next = controller
        .focused
        .map_or(ShellFocusState::NoFocus, ShellFocusState::Focused);
    if *state != next {
        *state = next;
    }
}

/// Keep the widget `Focused` marker on exactly the shell's focused control.
pub fn apply_shell_focus_marker(
    state: Res<ShellFocusState>,
    mut focusables: Query<(Entity, Has<Focused>), ShellFocusTargetFilter>,
    mut commands: Commands,
) {
    let target = state.focused_entity();
    for (entity, is_focused) in &mut focusables {
        let should_focus = target == Some(entity);
        if should_focus && !is_focused {
            commands.entity(entity).insert(Focused);
        } else if !should_focus && is_focused {
            commands.entity(entity).remove::<Focused>();
        }
    }
}

/// Install the shell focus controller, configuration, typed state, and systems.
pub struct ShellFocusPlugin;

impl Plugin for ShellFocusPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GamepadFocusController>();
        app.init_resource::<ShellFocusConfig>();
        app.init_resource::<ShellFocusState>();
        app.add_message::<ShellFocusNav>();
        app.add_systems(
            Update,
            (
                sync_shell_focus_targets,
                advance_shell_focus,
                sync_shell_focus_state,
                apply_shell_focus_marker,
            )
                .chain(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_focus_state_exposes_its_entity_only_when_focused() {
        let entity = Entity::from_raw_u32(7).expect("valid entity id");
        assert_eq!(ShellFocusState::default(), ShellFocusState::NoFocus);
        assert_eq!(ShellFocusState::NoFocus.focused_entity(), None);
        assert_eq!(
            ShellFocusState::Focused(entity).focused_entity(),
            Some(entity)
        );
    }

    #[test]
    fn default_config_wraps_with_the_shared_deadzone() {
        let config = ShellFocusConfig::default();
        assert!(config.wrap);
        assert_eq!(config.deadzone, SHELL_STICK_DEADZONE);
    }
}
