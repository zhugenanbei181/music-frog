//! Scroll area: the official unstyled `bevy_ui_widgets` [`ScrollArea`]
//! behavior wrapped in token container chrome. The official widget owns
//! wheel/trackpad semantics; this module adds typed touch-drag scrolling,
//! tap cancellation, bounded inertia, and raw pointer-cancel cleanup, and owns
//! the clipped viewport geometry and card surface.
//!
//! [`clamp_scroll`] is the pure projection the official observer applies
//! (delta applied, then clamped into the over-scroll range); it is kept
//! here as the headless-testable statement of that contract — the viewport
//! never scrolls past its content, and short content never scrolls at all.
//!
//! **Soft keyboard avoidance**: [`calculate_focus_avoidance_scroll`] and
//! [`focus_avoidance_auto_scroll_system`] ensure focused input fields stay
//! visible when an on-screen keyboard (mobile / Android / touch) appears.

use crate::palette::UiPalette;
use crate::palette::rem;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ecs::message::MessageReader;
use bevy::ecs::observer::On;
use bevy::ecs::query::{QueryData, With};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::math::Vec2;
use bevy::picking::events::{
    PointerCancel, PointerDrag, PointerDragEnd, PointerDragStart, PointerPress, PointerState,
};
use bevy::picking::pointer::{
    PointerAction, PointerButton, PointerId, PointerInput, PointerMap, PointerPressState,
};
use bevy::scene::{Scene, bsn};
use bevy::time::Time;
use bevy::transform::components::GlobalTransform;
use bevy::ui::BorderColor;
use bevy::ui::prelude::{
    BackgroundColor, BorderRadius, ComputedNode, FlexDirection, Node, Overflow, OverflowAxis,
    ScrollPosition, UiRect, Val, percent, px,
};
use bevy::ui::{Pressed, UiScale};
use bevy::ui_widgets::{Button, ScrollArea};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::iter;
use std::time::Duration;

/// Maximum speed, in logical UI pixels per second, applied to a released drag.
pub const SCROLL_INERTIA_MAX_SPEED: f32 = 2_400.0;
/// Speeds below this threshold stop instead of starting a barely visible coast.
pub const SCROLL_INERTIA_MIN_SPEED: f32 = 80.0;

const SCROLL_VELOCITY_SAMPLE_MAX_AGE: Duration = Duration::from_millis(150);
const SCROLL_DIRECT_STATE_TIMEOUT: Duration = Duration::from_millis(500);
const SCROLL_COAST_DECELERATION: f32 = 5.0;

/// Host preference for post-release motion.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScrollMotionPreference {
    /// Apply bounded inertial motion after a sufficiently fast drag.
    #[default]
    Full,
    /// Stop at the release position without inertial motion.
    Reduced,
}

/// Describes whether the latest pointer delta can be trusted for velocity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollVelocitySampling {
    /// A recent delta was converted to a velocity.
    Fresh { elapsed: Duration },
    /// A frame gap was too long to infer a fling safely.
    Stale { elapsed: Duration },
}

/// Typed motion state for one scroll area.
#[derive(Clone, Debug, PartialEq)]
pub enum ScrollMotion {
    /// A pointer currently owns a direct drag.
    Direct {
        pointer: PointerId,
        velocity: Vec2,
        sampling: ScrollVelocitySampling,
        last_sample: Duration,
        last_activity: Duration,
    },
    /// A released drag is slowing down under bounded inertia.
    Coasting { velocity: Vec2 },
}

/// Current gesture/coast state keyed by the scroll-area entity.
#[derive(Resource, Default, Debug)]
pub struct ScrollMotionState {
    motions: HashMap<Entity, ScrollMotion>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct PointerGestureKey {
    pointer: PointerId,
    button: PointerButton,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum PointerGesture {
    Pressed { controls: Vec<Entity> },
    Dragging { controls: Vec<Entity> },
}

/// Typed pointer/button state used to release controls on raw cancellation.
#[derive(Resource, Default, Debug)]
pub struct PointerGestureTracker {
    active: HashMap<PointerGestureKey, PointerGesture>,
}

impl ScrollMotionState {
    /// Read the current typed motion state for one area.
    pub fn motion(&self, area: Entity) -> Option<&ScrollMotion> {
        self.motions.get(&area)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ScrollDeltaOutcome {
    Applied,
    AtBoundary,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PointerContact {
    Pressed,
    Released,
    Unobserved,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DirectMotionLease {
    Current,
    Expired,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CoastDisposition {
    Continue,
    ReducedMotion,
    ReachedBoundary,
    BelowMinimumSpeed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReleaseDisposition {
    StartCoast,
    Stop,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ScrollBoundary {
    Lower,
    Upper,
    Interior,
}

fn classify_scroll_delta(previous: f32, next: f32) -> ScrollDeltaOutcome {
    match (next - previous).abs().partial_cmp(&f32::EPSILON) {
        Some(Ordering::Greater) => ScrollDeltaOutcome::Applied,
        Some(Ordering::Equal | Ordering::Less) | None => ScrollDeltaOutcome::AtBoundary,
    }
}

fn pointer_contact(
    pointer: PointerId,
    pointer_map: Option<&PointerMap>,
    pointer_states: &Query<&PointerPressState>,
) -> PointerContact {
    let Some(pointer_map) = pointer_map else {
        return PointerContact::Unobserved;
    };
    let Some(pointer_entity) = pointer_map.get_entity(pointer) else {
        return PointerContact::Released;
    };
    let Ok(press_state) = pointer_states.get(pointer_entity) else {
        return PointerContact::Released;
    };
    match press_state.is_primary_pressed() {
        true => PointerContact::Pressed,
        false => PointerContact::Released,
    }
}

fn direct_motion_lease(now: Duration, last_activity: Duration) -> DirectMotionLease {
    match now
        .saturating_sub(last_activity)
        .cmp(&SCROLL_DIRECT_STATE_TIMEOUT)
    {
        Ordering::Greater => DirectMotionLease::Expired,
        Ordering::Equal | Ordering::Less => DirectMotionLease::Current,
    }
}

fn coast_disposition(
    preference: ScrollMotionPreference,
    position: f32,
    max_scroll: f32,
    previous_position: f32,
    velocity: Vec2,
) -> CoastDisposition {
    let movement = classify_scroll_delta(previous_position, position);
    let boundary = match (
        position.partial_cmp(&0.0),
        position.partial_cmp(&max_scroll),
        velocity.y.partial_cmp(&0.0),
    ) {
        (Some(Ordering::Less | Ordering::Equal), _, Some(Ordering::Less)) => ScrollBoundary::Lower,
        (_, Some(Ordering::Greater | Ordering::Equal), Some(Ordering::Greater)) => {
            ScrollBoundary::Upper
        }
        _ => ScrollBoundary::Interior,
    };
    match (
        preference,
        movement,
        boundary,
        velocity.length().partial_cmp(&SCROLL_INERTIA_MIN_SPEED),
    ) {
        (ScrollMotionPreference::Reduced, _, _, _) => CoastDisposition::ReducedMotion,
        (_, _, ScrollBoundary::Lower | ScrollBoundary::Upper, _)
        | (_, ScrollDeltaOutcome::AtBoundary, _, _) => CoastDisposition::ReachedBoundary,
        (_, _, _, Some(Ordering::Less | Ordering::Equal) | None) => {
            CoastDisposition::BelowMinimumSpeed
        }
        (_, ScrollDeltaOutcome::Applied, ScrollBoundary::Interior, Some(Ordering::Greater)) => {
            CoastDisposition::Continue
        }
    }
}

fn nearest_scroll_area(
    target: Entity,
    parents: &Query<&ChildOf>,
    scroll_areas: &Query<(), With<ScrollArea>>,
) -> Option<Entity> {
    iter::once(target)
        .chain(parents.iter_ancestors(target))
        .find(|entity| scroll_areas.contains(*entity))
}

fn pressed_button_ancestors(
    target: Entity,
    parents: &Query<&ChildOf>,
    buttons: &Query<(), With<Button>>,
) -> Vec<Entity> {
    iter::once(target)
        .chain(parents.iter_ancestors(target))
        .filter(|entity| buttons.contains(*entity))
        .collect()
}

/// Begin tracking the nearest scroll area when a primary pointer starts a drag.
pub fn scrollarea_on_drag_start(
    drag: On<PointerDragStart>,
    parents: Query<&ChildOf>,
    scroll_areas: Query<(), With<ScrollArea>>,
    time: Res<Time>,
    mut motion: ResMut<ScrollMotionState>,
) {
    if drag.button != PointerButton::Primary {
        return;
    }
    let target = drag.original_event_target();
    let Some(area) = nearest_scroll_area(target, &parents, &scroll_areas) else {
        return;
    };
    let now = time.elapsed();
    motion.motions.insert(
        area,
        ScrollMotion::Direct {
            pointer: drag.pointer.id,
            velocity: Vec2::ZERO,
            sampling: ScrollVelocitySampling::Fresh {
                elapsed: Duration::ZERO,
            },
            last_sample: now,
            last_activity: now,
        },
    );
}

/// Record buttons held by a pointer so raw cancellation can release them.
pub fn track_button_pointer_press(
    press: On<PointerPress>,
    parents: Query<&ChildOf>,
    buttons: Query<(), With<Button>>,
    mut tracker: ResMut<PointerGestureTracker>,
) {
    let target = press.original_event_target();
    let controls = pressed_button_ancestors(target, &parents, &buttons);
    if controls.is_empty() {
        return;
    }
    let key = PointerGestureKey {
        pointer: press.pointer.id,
        button: press.button,
    };
    let mut held_controls = match tracker.active.remove(&key) {
        Some(PointerGesture::Pressed { controls })
        | Some(PointerGesture::Dragging { controls }) => controls,
        None => Vec::new(),
    };
    for control in controls {
        if !held_controls.contains(&control) {
            held_controls.push(control);
        }
    }
    tracker.active.insert(
        key,
        PointerGesture::Pressed {
            controls: held_controls,
        },
    );
}

/// A recognized drag ends tap eligibility for this pointer/button pair.
/// Bevy 0.20 still emits `PointerClick` on release after a drag when the same
/// entity remains hovered; clearing the Picking press set at drag start keeps
/// that low-level click from activating a control.
pub fn cancel_pointer_tap_on_drag_start(
    drag: On<PointerDragStart>,
    pointer_state: Option<ResMut<PointerState>>,
    mut tracker: ResMut<PointerGestureTracker>,
) {
    let key = PointerGestureKey {
        pointer: drag.pointer.id,
        button: drag.button,
    };
    match tracker.active.remove(&key) {
        Some(PointerGesture::Pressed { controls })
        | Some(PointerGesture::Dragging { controls }) => {
            tracker
                .active
                .insert(key, PointerGesture::Dragging { controls });
        }
        None => (),
    }
    if let Some(mut pointer_state) = pointer_state {
        pointer_state
            .get_mut(drag.pointer.id, drag.button)
            .pressing
            .clear();
    }
}

/// Apply direct pointer movement to the nearest scrollable vertical viewport.
pub fn scrollarea_on_drag(
    mut drag: On<PointerDrag>,
    parents: Query<&ChildOf>,
    scroll_areas: Query<(), With<ScrollArea>>,
    mut viewports: Query<(&Node, &ComputedNode, &mut ScrollPosition), With<ScrollArea>>,
    time: Res<Time>,
    ui_scale: Res<UiScale>,
    mut motion: ResMut<ScrollMotionState>,
) {
    if drag.button != PointerButton::Primary {
        return;
    }
    let Some(area) = nearest_scroll_area(drag.original_event_target(), &parents, &scroll_areas)
    else {
        return;
    };
    let Some(ScrollMotion::Direct {
        pointer,
        last_sample,
        ..
    }) = motion.motions.get(&area).cloned()
    else {
        return;
    };
    if pointer != drag.pointer.id {
        return;
    }
    let Ok((node, computed, mut position)) = viewports.get_mut(area) else {
        return;
    };
    if node.overflow.y != OverflowAxis::Scroll {
        return;
    }

    let scale = ui_scale.0.max(f32::EPSILON);
    let delta_y = drag.delta.y / scale;
    if !delta_y.is_finite() {
        return;
    }
    let visible_height = computed.size().y * computed.inverse_scale_factor;
    let content_height = computed.content_size().y * computed.inverse_scale_factor;
    let max_scroll = (content_height - visible_height).max(0.0);
    let previous = position.y;
    position.y = (position.y - delta_y).clamp(0.0, max_scroll);
    let delta_outcome = classify_scroll_delta(previous, position.y);

    let now = time.elapsed();
    let elapsed = now.saturating_sub(last_sample);
    let sampling = match elapsed {
        elapsed if elapsed.is_zero() || elapsed > SCROLL_VELOCITY_SAMPLE_MAX_AGE => {
            ScrollVelocitySampling::Stale { elapsed }
        }
        elapsed => ScrollVelocitySampling::Fresh { elapsed },
    };
    let velocity = match sampling {
        ScrollVelocitySampling::Fresh { elapsed } => {
            let velocity = Vec2::new(0.0, -delta_y / elapsed.as_secs_f32());
            let speed = velocity.length();
            if speed > SCROLL_INERTIA_MAX_SPEED {
                velocity * (SCROLL_INERTIA_MAX_SPEED / speed)
            } else {
                velocity
            }
        }
        ScrollVelocitySampling::Stale { .. } => Vec2::ZERO,
    };
    motion.motions.insert(
        area,
        ScrollMotion::Direct {
            pointer,
            velocity,
            sampling,
            last_sample: now,
            last_activity: now,
        },
    );
    match delta_outcome {
        ScrollDeltaOutcome::Applied => drag.propagate(false),
        ScrollDeltaOutcome::AtBoundary => (),
    }
}

/// End direct tracking and begin a coast only when the last sample is recent.
pub fn scrollarea_on_drag_end(
    drag: On<PointerDragEnd>,
    parents: Query<&ChildOf>,
    scroll_areas: Query<(), With<ScrollArea>>,
    time: Res<Time>,
    preference: Res<ScrollMotionPreference>,
    mut motion: ResMut<ScrollMotionState>,
) {
    if drag.button != PointerButton::Primary {
        return;
    }
    let Some(area) = nearest_scroll_area(drag.original_event_target(), &parents, &scroll_areas)
    else {
        return;
    };
    let Some(ScrollMotion::Direct {
        pointer,
        velocity,
        sampling,
        last_sample,
        ..
    }) = motion.motions.get(&area).cloned()
    else {
        return;
    };
    if pointer != drag.pointer.id {
        return;
    }
    let elapsed = time.elapsed().saturating_sub(last_sample);
    let release_disposition = match (
        *preference,
        sampling,
        elapsed.cmp(&SCROLL_VELOCITY_SAMPLE_MAX_AGE),
        velocity.length().partial_cmp(&SCROLL_INERTIA_MIN_SPEED),
    ) {
        (
            ScrollMotionPreference::Full,
            ScrollVelocitySampling::Fresh { .. },
            Ordering::Less | Ordering::Equal,
            Some(Ordering::Greater),
        ) => ReleaseDisposition::StartCoast,
        _ => ReleaseDisposition::Stop,
    };
    match release_disposition {
        ReleaseDisposition::StartCoast => {
            motion
                .motions
                .insert(area, ScrollMotion::Coasting { velocity });
        }
        ReleaseDisposition::Stop => {
            motion.motions.remove(&area);
        }
    }
}

/// Cancel direct motion without creating a fling.
pub fn scrollarea_on_drag_cancel(
    cancel: On<PointerCancel>,
    parents: Query<&ChildOf>,
    scroll_areas: Query<(), With<ScrollArea>>,
    mut motion: ResMut<ScrollMotionState>,
) {
    let Some(area) = nearest_scroll_area(cancel.original_event_target(), &parents, &scroll_areas)
    else {
        return;
    };
    match motion.motions.get(&area).cloned() {
        Some(ScrollMotion::Direct { pointer, .. }) if pointer == cancel.pointer.id => {
            motion.motions.remove(&area);
        }
        Some(ScrollMotion::Direct { .. }) | Some(ScrollMotion::Coasting { .. }) | None => (),
    }
}

/// Apply raw pointer lifecycle events that Picking cannot always retarget to a UI node.
pub fn cancel_motion_on_pointer_input(
    mut pointer_inputs: MessageReader<PointerInput>,
    mut commands: Commands,
    mut motion: ResMut<ScrollMotionState>,
    mut gestures: ResMut<PointerGestureTracker>,
) {
    for input in pointer_inputs.read() {
        let gesture_keys = match input.action {
            PointerAction::Cancel => gestures
                .active
                .keys()
                .filter_map(|key| match key.pointer == input.pointer_id {
                    true => Some(*key),
                    false => None,
                })
                .collect::<Vec<_>>(),
            PointerAction::Release(button) => vec![PointerGestureKey {
                pointer: input.pointer_id,
                button,
            }],
            PointerAction::Press(_) | PointerAction::Move { .. } | PointerAction::Scroll { .. } => {
                Vec::new()
            }
        };
        for key in gesture_keys {
            if let Some(gesture) = gestures.active.remove(&key) {
                let controls = match gesture {
                    PointerGesture::Pressed { controls }
                    | PointerGesture::Dragging { controls } => controls,
                };
                for control in controls {
                    commands.entity(control).remove::<Pressed>();
                }
            }
        }

        match input.action {
            PointerAction::Cancel | PointerAction::Release(_) => {
                let ended_areas: Vec<Entity> = motion
                    .motions
                    .iter()
                    .filter_map(|(area, state)| match state {
                        ScrollMotion::Direct { pointer, .. } if *pointer == input.pointer_id => {
                            Some(*area)
                        }
                        ScrollMotion::Direct { .. } | ScrollMotion::Coasting { .. } => None,
                    })
                    .collect();
                for area in ended_areas {
                    motion.motions.remove(&area);
                }
            }
            PointerAction::Press(_) | PointerAction::Move { .. } | PointerAction::Scroll { .. } => {
            }
        }
    }
}

/// Advance inertia, clamp it to the live content geometry, and prune stale states.
pub fn advance_scroll_motion(
    time: Res<Time>,
    preference: Res<ScrollMotionPreference>,
    pointer_map: Option<Res<PointerMap>>,
    pointer_states: Query<&PointerPressState>,
    mut motion: ResMut<ScrollMotionState>,
    mut viewports: Query<(&Node, &ComputedNode, &mut ScrollPosition), With<ScrollArea>>,
) {
    let now = time.elapsed();
    let delta_seconds = time.delta_secs();
    let areas: Vec<Entity> = motion.motions.keys().copied().collect();
    for area in areas {
        let Some(state) = motion.motions.get(&area).cloned() else {
            continue;
        };
        let Ok((node, computed, mut position)) = viewports.get_mut(area) else {
            motion.motions.remove(&area);
            continue;
        };
        let visible_height = computed.size().y * computed.inverse_scale_factor;
        let content_height = computed.content_size().y * computed.inverse_scale_factor;
        let max_scroll = (content_height - visible_height).max(0.0);
        match state {
            ScrollMotion::Direct {
                pointer,
                last_activity,
                ..
            } => {
                let lease = direct_motion_lease(now, last_activity);
                let contact = pointer_contact(pointer, pointer_map.as_deref(), &pointer_states);
                match (lease, contact) {
                    (_, PointerContact::Pressed) | (DirectMotionLease::Current, _) => (),
                    (
                        DirectMotionLease::Expired,
                        PointerContact::Released | PointerContact::Unobserved,
                    ) => {
                        motion.motions.remove(&area);
                    }
                }
            }
            ScrollMotion::Coasting { velocity } => {
                if node.overflow.y != OverflowAxis::Scroll {
                    motion.motions.remove(&area);
                    continue;
                }
                let previous = position.y;
                position.y = (position.y + velocity.y * delta_seconds).clamp(0.0, max_scroll);
                let next_velocity = velocity * (-SCROLL_COAST_DECELERATION * delta_seconds).exp();
                let disposition =
                    coast_disposition(*preference, position.y, max_scroll, previous, next_velocity);
                match disposition {
                    CoastDisposition::Continue => {
                        motion.motions.insert(
                            area,
                            ScrollMotion::Coasting {
                                velocity: next_velocity,
                            },
                        );
                    }
                    CoastDisposition::ReducedMotion
                    | CoastDisposition::ReachedBoundary
                    | CoastDisposition::BelowMinimumSpeed => {
                        motion.motions.remove(&area);
                    }
                }
            }
        }
    }
}

/// Virtual / soft keyboard state (e.g. on Android / mobile / touch screens).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub enum SoftKeyboardState {
    /// The IME is not visible.
    #[default]
    Closed,
    /// The IME occupies this many logical vertical pixels.
    Open { height_px: f32 },
}

/// Marker component placed on the focused text input entity requiring soft keyboard avoidance.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FocusedTextInput;

/// Geometry parameters for calculating soft keyboard auto-scroll avoidance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FocusAvoidanceParams {
    /// Top coordinate (Y) of the scroll viewport in screen / window coordinates.
    pub viewport_top: f32,
    /// Height of the scroll viewport in pixels.
    pub viewport_height: f32,
    /// Total scrollable content height in pixels.
    pub content_height: f32,
    /// Current vertical scroll offset (0.0 means scrolled to top).
    pub current_scroll: f32,
    /// Top coordinate (Y) of the focused input widget in screen / window coordinates.
    pub target_top: f32,
    /// Height of the focused input widget in pixels.
    pub target_height: f32,
    /// Height of the soft keyboard in pixels (0.0 if closed).
    pub keyboard_height: f32,
    /// Safety comfort margin above the keyboard in pixels (e.g. 16.0 px).
    pub margin_px: f32,
}

#[derive(QueryData)]
#[query_data(mutable)]
pub struct ScrollViewport {
    scroll_transform: Option<&'static GlobalTransform>,
    scroll_node: Option<&'static ComputedNode>,
    scroll_pos: &'static mut ScrollPosition,
    children: Option<&'static Children>,
}

/// Calculate the updated scroll offset so the focused input is not occluded by the soft keyboard.
/// Pure, total, clamped into `[0, max_scroll]`.
pub fn calculate_focus_avoidance_scroll(params: FocusAvoidanceParams) -> f32 {
    if !params.keyboard_height.is_finite()
        || params.keyboard_height <= 0.0
        || params.viewport_height <= 0.0
    {
        return params.current_scroll;
    }

    let visible_viewport_h = (params.viewport_height - params.keyboard_height).max(0.0);
    if visible_viewport_h <= 0.0 {
        return params.current_scroll;
    }

    let target_bottom = params.target_top + params.target_height;
    let visible_bottom = params.viewport_top + visible_viewport_h;
    let visible_top = params.viewport_top;

    let mut new_scroll = params.current_scroll;

    // If target bottom is below visible bottom (occluded or too close to keyboard)
    if target_bottom + params.margin_px > visible_bottom {
        let overflow = (target_bottom + params.margin_px) - visible_bottom;
        new_scroll += overflow;
    }
    // If target top is above visible top (scrolled too far down)
    else if params.target_top - params.margin_px < visible_top {
        let underflow = visible_top - (params.target_top - params.margin_px);
        new_scroll = (new_scroll - underflow).max(0.0);
    }

    clamp_scroll(new_scroll, params.content_height, params.viewport_height)
}

/// ECS system: automatically adjusts `ScrollPosition` on `ScrollArea` nodes
/// when a focused input field is occluded by the soft keyboard.
pub fn focus_avoidance_auto_scroll_system(
    keyboard: Option<Res<SoftKeyboardState>>,
    focused_inputs: Query<
        (Option<&GlobalTransform>, Option<&ComputedNode>),
        With<FocusedTextInput>,
    >,
    mut scroll_areas: Query<ScrollViewport, With<ScrollArea>>,
    node_query: Query<(&ComputedNode, Option<&Children>)>,
) {
    let keyboard_height = match keyboard.as_deref() {
        Some(SoftKeyboardState::Open { height_px }) => *height_px,
        Some(SoftKeyboardState::Closed) | None => return,
    };
    if !keyboard_height.is_finite() || keyboard_height <= 0.0 {
        return;
    }

    let Some((target_transform, target_node)) = focused_inputs.iter().next() else {
        return;
    };

    let target_size = target_node
        .map(|n| n.size())
        .unwrap_or(Vec2::new(200.0, 44.0));
    let target_top = target_transform
        .map(|t| t.translation().y - target_size.y * 0.5)
        .unwrap_or(0.0);

    for ScrollViewportItem {
        scroll_transform,
        scroll_node,
        mut scroll_pos,
        children,
    } in &mut scroll_areas
    {
        let viewport_size = scroll_node
            .map(|n| n.size())
            .unwrap_or(Vec2::new(300.0, 400.0));
        let viewport_top = scroll_transform
            .map(|t| t.translation().y - viewport_size.y * 0.5)
            .unwrap_or(0.0);

        let mut content_height = viewport_size.y;
        if let Some(children) = children {
            for child in children.iter() {
                if let Ok((child_node, _)) = node_query.get(*child) {
                    content_height = content_height.max(child_node.size().y);
                }
            }
        }

        let params = FocusAvoidanceParams {
            viewport_top,
            viewport_height: viewport_size.y,
            content_height,
            current_scroll: scroll_pos.0.y,
            target_top,
            target_height: target_size.y,
            keyboard_height,
            margin_px: 16.0,
        };

        let target_scroll = calculate_focus_avoidance_scroll(params);
        if (scroll_pos.0.y - target_scroll).abs() > 0.5 {
            scroll_pos.0.y = target_scroll;
        }
    }
}

/// Clamp a scroll offset into the legal range for a viewport: at most
/// `content - viewport` (never less than zero), and zero entirely when the
/// content fits. Mirrors the official `ScrollArea` observer's clamp.
pub fn clamp_scroll(position: f32, content: f32, viewport: f32) -> f32 {
    position.clamp(0.0, (content - viewport).max(0.0))
}

/// A token card hosting one scrollable content scene. The viewport is
/// vertically scrollable and clips at the card's rounded edge; the caller
/// owns the viewport height (their layout decides) and the content scene.
pub fn scrollarea_scene(
    content: Box<dyn Scene>,
    viewport_height_px: f32,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let edge = palette.border;
    bsn! {
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Column,
                border: UiRect::all(Val::Px(palette.hairline_px)),
                border_radius: BorderRadius::all(rem(palette.card_radius_px)),
            }
            BackgroundColor({ palette.surface })
            BorderColor {
                top: edge,
                right: edge,
                bottom: edge,
                left: edge,
            }
            Children [
                Node {
                    width: percent(100),
                    height: px(viewport_height_px),
                    flex_direction: FlexDirection::Column,
                    overflow: Overflow::scroll_y(),
                }
                ScrollArea
                Children [
                    @{ content }
                ]
            ]
    }
}
