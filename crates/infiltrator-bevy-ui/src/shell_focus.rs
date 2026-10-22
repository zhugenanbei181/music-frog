//! Shell focus: the official input focus is the only focus truth.
//!
//! The official `bevy_input_focus::InputFocus` says which control holds the
//! focus; the shell only *projects* it. The visible focus — the widget
//! `Focused` marker and the focus ring — is derived from the official focus
//! and from the official focus cause, so the shell cannot drift from the
//! official ledger. The shell keeps one thing of its own: the *order* of the
//! measured focusables (screen reading order). The order says which control
//! comes next; it never says which control is focused.
//!
//! The movement is the official one: the arrow-key gesture (owned by the
//! widget layer's directional navigation) and the shell's D-pad both go
//! through Bevy 0.20's official directional navigation, which measures the
//! mounted controls and moves the official focus. The shell's own edge policy
//! — the wrap around — is the only spatial rule left, and it is a fallback
//! for "the official navigator found no neighbour in that direction", not a
//! second neighbour search.

use crate::route::NavigateBack;
use bevy::app::{App, Plugin, PostUpdate, Update};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::message::{Message, MessageReader};
use bevy::ecs::observer::On;
use bevy::ecs::query::{Has, Or, With};
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::input::ButtonInput;
use bevy::input::keyboard::KeyCode;
use bevy::input_focus::tab_navigation::TabIndex;
use bevy::input_focus::{FocusCause, FocusGained, InputFocus, InputFocusSystems};
use bevy::math::Vec2;
use bevy::scene::CommandsSceneExt;
use bevy::ui::auto_directional_navigation::{AutoDirectionalNavigation, AutoDirectionalNavigator};
use bevy::ui::{ComputedNode, UiGlobalTransform};
use bevy::ui_widgets::{Activate, Button, ModalDialog};
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::focus::{Focused, focus_ring_scene};
use infiltrator_bevy_widgets::gamepad_ui::{GamepadDirection, GamepadNavAction};
use infiltrator_bevy_widgets::palette::UiPalette;

/// Marker on every shell control reachable by focus.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ShellFocusable;

/// The focus ring drawn over the focused control.
///
/// The ring carries the control it decorates, so the shell knows when the
/// focus moved and the ring has to follow.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShellFocusRing(pub Entity);

/// Shell focus targets: the explicit shell markers plus every mounted button, so
/// focus reaches page content without each page re-declaring the seam.
type ShellFocusTargetFilter = Or<(With<ShellFocusable>, With<Button>)>;

/// A measured focusable control: the entity and the centre of its measured box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShellFocusTarget {
    pub entity: Entity,
    pub center: Vec2,
}

/// The shell's focus order: the measured focusables in screen reading order.
///
/// This is an order, not a focus. The official `InputFocus` says which
/// control holds the focus; this list only says which control comes next.
/// The controls inside an official modal dialog are not in it: the modal owns
/// the focus inside its own trap.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct ShellFocusOrder {
    pub targets: Vec<ShellFocusTarget>,
}

impl ShellFocusOrder {
    /// Whether the entity is one of the shell's own focusables.
    pub fn contains(&self, entity: Entity) -> bool {
        self.targets.iter().any(|target| target.entity == entity)
    }
}

/// Default radial deadzone applied to analog-stick deflection.
pub const SHELL_STICK_DEADZONE: f32 = 0.15;

/// Perpendicular distance within which two controls count as the same row or
/// column while the focus wraps around the edge.
const WRAP_BAND_TOLERANCE_PX: f32 = 8.0;

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

/// Typed shell focus: the official focus, projected.
///
/// The projection is derived from the official `InputFocus`; no gesture writes
/// it, so it cannot drift from the official truth. "Nothing is focused" stays
/// an explicit state rather than an absent entity.
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

/// The cause of the official focus, as the official layer reported it.
///
/// The official `InputFocus` does not expose the cause of the current focus,
/// so the shell reads it from the official `FocusGained` event.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ShellFocusCause(pub Option<FocusCause>);

impl ShellFocusCause {
    /// Whether the official focus was reached by the user rather than by the
    /// shell's own automatic landing.
    pub fn is_user_driven(self) -> bool {
        matches!(self.0, Some(FocusCause::Navigated | FocusCause::Pressed))
    }
}

/// A gamepad navigation request. A host adapter forwards D-pad/face-button
/// actions and raw analog-stick samples; the shell never reads a device itself.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub enum ShellFocusNav {
    Action(GamepadNavAction),
    Stick(Vec2),
}

/// The wrap request: the official navigator found no neighbour in that
/// direction, so the shell asks for the far end of the row or column.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct ShellFocusWrap {
    /// The control the official focus sits on.
    pub from: Entity,
    /// The direction that had no neighbour.
    pub direction: GamepadDirection,
}

/// Measure the mounted focusables into the shell's focus order.
///
/// A focusable that is disabled, hidden, or not yet laid out (no finite,
/// positive measured box) is not reachable, so it is skipped. A focusable
/// inside an official modal dialog is not a shell focusable either: the modal
/// owns the focus inside its own trap. The order is screen reading order —
/// top to bottom, then left to right — and entity bits break exact-position
/// ties deterministically within the current world.
pub fn sync_shell_focus_order(
    mut order: ResMut<ShellFocusOrder>,
    focusables: Query<
        (
            Entity,
            &ComputedNode,
            &UiGlobalTransform,
            Option<&ButtonDisabled>,
        ),
        ShellFocusTargetFilter,
    >,
    parents: Query<&ChildOf>,
    dialogs: Query<(), With<ModalDialog>>,
) {
    let mut measured: Vec<ShellFocusTarget> = focusables
        .iter()
        .filter(|(entity, ..)| !inside_modal(*entity, &parents, &dialogs))
        .filter_map(|(entity, computed, transform, disabled)| {
            let size = computed.size();
            let center = transform.affine().translation;
            let reachable = size.is_finite()
                && size.x > 0.0
                && size.y > 0.0
                && center.is_finite()
                && !disabled.is_some_and(|d| d.0);
            reachable.then_some(ShellFocusTarget { entity, center })
        })
        .collect();
    measured.sort_by(|left, right| {
        left.center
            .y
            .total_cmp(&right.center.y)
            .then_with(|| left.center.x.total_cmp(&right.center.x))
            .then_with(|| left.entity.to_bits().cmp(&right.entity.to_bits()))
    });
    order.targets = measured;
}

/// Whether the control sits inside an official modal dialog.
///
/// The official `ModalDialog` is a focus trap: the focus belongs to the modal
/// while it is open, so the shell neither lands its focus there nor draws its
/// ring over the modal's focused control.
fn inside_modal(
    entity: Entity,
    parents: &Query<&ChildOf>,
    dialogs: &Query<(), With<ModalDialog>>,
) -> bool {
    let mut current = entity;
    while let Ok(parent) = parents.get(current) {
        current = parent.parent();
        if dialogs.contains(current) {
            return true;
        }
    }
    false
}

/// Land the official focus on a shell control.
///
/// The official navigator moves the focus *from* a focused control, so the
/// shell keeps one: when the official focus does not sit on a shell control
/// (nothing holds it, or it holds the window), the shell lands it on the
/// first measured control. A modal that owns the focus keeps it: the landing
/// never takes the focus out of an open dialog. The landing is an
/// *automatic* focus, so the route-only baseline stays invisible: the ring
/// marks a focus the user moved.
pub fn land_shell_focus(
    order: Res<ShellFocusOrder>,
    mut focus: ResMut<InputFocus>,
    focusables: Query<(), ShellFocusTargetFilter>,
    parents: Query<&ChildOf>,
    dialogs: Query<(), With<ModalDialog>>,
) {
    let Some(first) = order.targets.first() else {
        return;
    };
    if let Some(current) = focus.get()
        && (focusables.contains(current) || inside_modal(current, &parents, &dialogs))
    {
        return;
    }
    focus.set(first.entity, FocusCause::Auto);
}

/// Handle each navigation request: directional actions and stick samples move
/// the official focus through the official directional navigation; confirm
/// activates the focused control; cancel navigates back.
pub fn advance_shell_focus(
    mut navs: MessageReader<ShellFocusNav>,
    config: Res<ShellFocusConfig>,
    mut navigator: AutoDirectionalNavigator,
    mut commands: Commands,
) {
    for nav in navs.read() {
        match *nav {
            ShellFocusNav::Action(GamepadNavAction::ButtonAConfirm) => {
                if let Some(entity) = navigator.input_focus() {
                    commands.trigger(Activate { entity });
                }
            }
            ShellFocusNav::Action(GamepadNavAction::ButtonBCancel) => {
                commands.trigger(NavigateBack);
            }
            ShellFocusNav::Action(action) => {
                if let Some(direction) = GamepadDirection::from_action(action) {
                    navigate_official(&mut navigator, direction, &mut commands);
                }
            }
            ShellFocusNav::Stick(stick) => {
                if let Some(direction) = GamepadDirection::from_stick(stick, config.deadzone) {
                    navigate_official(&mut navigator, direction, &mut commands);
                }
            }
        }
    }
}

/// Move the official focus one step in `direction`.
///
/// The official navigator owns the neighbour search: it measures the mounted
/// controls and moves the official input focus. When it finds no neighbour the
/// shell may wrap around the edge, and it says so with a wrap request.
fn navigate_official(
    navigator: &mut AutoDirectionalNavigator,
    direction: GamepadDirection,
    commands: &mut Commands,
) {
    let Some(current) = navigator.input_focus() else {
        return;
    };
    if navigator.navigate(direction.compass_octant()).is_ok() {
        return;
    }
    commands.write_message(ShellFocusWrap {
        from: current,
        direction,
    });
}

/// Wrap the focus around the edge when the official navigator found no
/// neighbour in that direction.
///
/// The wrap target is the far end of the row or column the focus sits in: the
/// shell's own edge policy, expressed over the measured order. Every
/// neighbour search stays with the official navigator.
pub fn apply_shell_focus_wrap(
    mut wraps: MessageReader<ShellFocusWrap>,
    order: Res<ShellFocusOrder>,
    config: Res<ShellFocusConfig>,
    mut focus: ResMut<InputFocus>,
) {
    if !config.wrap {
        return;
    }
    for wrap in wraps.read() {
        let Some(target) = wrap_target(&order.targets, wrap.from, wrap.direction) else {
            continue;
        };
        focus.set(target, FocusCause::Navigated);
    }
}

/// The far end of the row or column the focus sits in.
///
/// The perpendicular band keeps the wrap inside the row (or column) the focus
/// is in; when the focus has no band-mate, the far end of the whole layout is
/// the wrap target.
fn wrap_target(
    targets: &[ShellFocusTarget],
    from: Entity,
    direction: GamepadDirection,
) -> Option<Entity> {
    let current = targets.iter().find(|target| target.entity == from)?;
    let axis = direction.unit_axis();
    let perpendicular = Vec2::new(-axis.y, axis.x);
    let in_band = |target: &ShellFocusTarget| {
        (target.center - current.center).dot(perpendicular).abs() <= WRAP_BAND_TOLERANCE_PX
    };
    let extreme = |only_band: bool| -> Option<Entity> {
        targets
            .iter()
            .filter(|target| target.entity != current.entity && (!only_band || in_band(target)))
            .min_by(|left, right| left.center.dot(axis).total_cmp(&right.center.dot(axis)))
            .map(|target| target.entity)
    };
    extreme(true).or_else(|| extreme(false))
}

/// Move the shell focus with the keyboard's Tab / Shift+Tab gesture.
///
/// The gesture walks the shell's focus order and hands the result to the
/// official focus, so the visible focus stays the official one.
pub fn advance_shell_focus_by_keyboard(
    keyboard: Option<Res<ButtonInput<KeyCode>>>,
    order: Res<ShellFocusOrder>,
    mut focus: ResMut<InputFocus>,
) {
    let Some(keyboard) = keyboard else {
        return;
    };
    if !keyboard.just_pressed(KeyCode::Tab) {
        return;
    }
    let backwards = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
    let Some(target) = next_in_order(&order.targets, focus.get(), backwards) else {
        return;
    };
    focus.set(target, FocusCause::Navigated);
}

/// The next control in the shell's focus order, wrapping around the ends.
fn next_in_order(
    targets: &[ShellFocusTarget],
    current: Option<Entity>,
    backwards: bool,
) -> Option<Entity> {
    let Some(first) = targets.first() else {
        return None;
    };
    let Some(current) = current else {
        return Some(first.entity);
    };
    let Some(index) = targets.iter().position(|target| target.entity == current) else {
        return None;
    };
    let next = if backwards {
        (index + targets.len() - 1) % targets.len()
    } else {
        (index + 1) % targets.len()
    };
    Some(targets[next].entity)
}

/// Publish the official focus as the typed shell focus state.
pub fn sync_shell_focus_state(focus: Res<InputFocus>, mut state: ResMut<ShellFocusState>) {
    let next = focus
        .get()
        .map_or(ShellFocusState::NoFocus, ShellFocusState::Focused);
    if *state != next {
        *state = next;
    }
}

/// Keep the widget `Focused` marker on exactly the officially focused control.
pub fn apply_shell_focus_marker(
    focus: Res<InputFocus>,
    order: Res<ShellFocusOrder>,
    mut focusables: Query<(Entity, Has<Focused>), ShellFocusTargetFilter>,
    mut commands: Commands,
) {
    let target = focus.get().filter(|entity| order.contains(*entity));
    for (entity, is_focused) in &mut focusables {
        let should_focus = target == Some(entity);
        if should_focus && !is_focused {
            commands.entity(entity).insert(Focused);
        } else if !should_focus && is_focused {
            commands.entity(entity).remove::<Focused>();
        }
    }
}

/// Draw the shared focus ring over the focused control.
///
/// The ring marks a focus the *user* moved: the shell's own landing on the
/// first measured control is the route-only baseline (an automatic focus), and
/// a gesture — the arrow keys, the D-pad, the Tab key or a pointer press —
/// that moves the official focus is what the ring makes visible. The ring
/// decorates the shell's own controls; a modal owns the focus inside its own
/// trap and draws its own.
pub fn apply_shell_focus_ring(
    focus: Res<InputFocus>,
    cause: Res<ShellFocusCause>,
    order: Res<ShellFocusOrder>,
    palette: Res<UiPalette>,
    mut commands: Commands,
    rings: Query<(Entity, &ShellFocusRing)>,
) {
    let target = focus
        .get()
        .filter(|entity| order.contains(*entity))
        .filter(|_| cause.is_user_driven());
    let existing = rings.iter().next();
    if let Some((entity, ring)) = existing {
        if Some(ring.0) == target {
            return;
        }
        commands.entity(entity).despawn();
    }
    let Some(target) = target else {
        return;
    };
    commands
        .spawn_scene(focus_ring_scene(&palette))
        .insert(ChildOf(target))
        .insert(ShellFocusRing(target));
}

/// Record the cause of the official focus.
///
/// The official `InputFocus` does not expose the cause of the current focus,
/// so the shell reads it from the official `FocusGained` event: the official
/// layer says *why* the focus moved, and the visible focus follows that.
pub fn record_shell_focus_cause(gained: On<FocusGained>, mut cause: ResMut<ShellFocusCause>) {
    cause.0 = Some(gained.cause);
}

/// Install the shell focus wiring: the official focus components, the typed
/// state, the focus order and the systems.
pub struct ShellFocusPlugin;

impl Plugin for ShellFocusPlugin {
    fn build(&self, app: &mut App) {
        // The official focus components attach at spawn, before any
        // navigation can move the focus: the official pointer and tab focus
        // reach a control that carries a `TabIndex`, and the official
        // directional navigation reaches a control that carries
        // `AutoDirectionalNavigation`.
        // The widget layer owns the button's official tab index; registering
        // it twice is a duplicate registration and Bevy refuses it.
        app.register_required_components::<ShellFocusable, TabIndex>()
            .register_required_components::<ShellFocusable, AutoDirectionalNavigation>();
        app.init_resource::<ShellFocusConfig>();
        app.init_resource::<ShellFocusState>();
        app.init_resource::<ShellFocusCause>();
        app.init_resource::<ShellFocusOrder>();
        app.add_message::<ShellFocusNav>();
        app.add_message::<ShellFocusWrap>();
        app.add_observer(record_shell_focus_cause);
        app.add_systems(
            Update,
            (
                sync_shell_focus_order,
                land_shell_focus,
                advance_shell_focus,
                apply_shell_focus_wrap,
                advance_shell_focus_by_keyboard,
            )
                .chain(),
        );
        // The visible focus is projected after the official focus-change
        // events, so the official focus *cause* is the one of the current
        // focus and not the one of the previous frame.
        app.add_systems(
            PostUpdate,
            (
                sync_shell_focus_state,
                apply_shell_focus_marker,
                apply_shell_focus_ring,
            )
                .chain()
                .after(InputFocusSystems::FocusChangeEvents),
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

    #[test]
    fn only_a_user_driven_focus_shows_the_ring() {
        assert!(!ShellFocusCause::default().is_user_driven());
        assert!(!ShellFocusCause(Some(FocusCause::Auto)).is_user_driven());
        assert!(ShellFocusCause(Some(FocusCause::Navigated)).is_user_driven());
        assert!(ShellFocusCause(Some(FocusCause::Pressed)).is_user_driven());
    }

    #[test]
    fn the_focus_order_walks_the_controls_in_reading_order() {
        let first = Entity::from_raw_u32(1).expect("valid entity id");
        let second = Entity::from_raw_u32(2).expect("valid entity id");
        let targets = vec![
            ShellFocusTarget {
                entity: first,
                center: Vec2::new(0.0, 0.0),
            },
            ShellFocusTarget {
                entity: second,
                center: Vec2::new(100.0, 0.0),
            },
        ];

        assert_eq!(next_in_order(&targets, None, false), Some(first));
        assert_eq!(next_in_order(&targets, Some(first), false), Some(second));
        assert_eq!(next_in_order(&targets, Some(second), false), Some(first));
        assert_eq!(next_in_order(&targets, Some(second), true), Some(first));
        assert_eq!(next_in_order(&targets, Some(first), true), Some(second));
    }

    #[test]
    fn the_wrap_target_is_the_far_end_of_the_same_row() {
        let left = Entity::from_raw_u32(1).expect("valid entity id");
        let right = Entity::from_raw_u32(2).expect("valid entity id");
        let bottom = Entity::from_raw_u32(3).expect("valid entity id");
        let targets = vec![
            ShellFocusTarget {
                entity: left,
                center: Vec2::new(0.0, 0.0),
            },
            ShellFocusTarget {
                entity: right,
                center: Vec2::new(100.0, 0.0),
            },
            ShellFocusTarget {
                entity: bottom,
                center: Vec2::new(0.0, 50.0),
            },
        ];

        assert_eq!(
            wrap_target(&targets, right, GamepadDirection::Right),
            Some(left),
            "the wrap stays inside the row the focus sits in"
        );
        assert_eq!(
            wrap_target(&targets, left, GamepadDirection::Down),
            Some(bottom)
        );
    }
}
