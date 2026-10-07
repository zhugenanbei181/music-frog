//! Animation easing curves, spring physics simulation, and color interpolation.

use bevy::app::{App, Plugin, Update};
use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::query::QueryData;
use bevy::ecs::system::{Query, Res};
use bevy::math::{Vec2, Vec3};
use bevy::picking::hover::PickingInteraction;
use bevy::time::Time;
use bevy::transform::components::Transform;
use bevy::ui::Node;
use bevy::ui::prelude::Val;

/// Standard cubic-bezier and procedural easing curves.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Easing {
    #[default]
    Linear,
    EaseInQuad,
    EaseOutQuad,
    EaseInOutQuad,
    EaseOutCubic,
    EaseInOutCubic,
    EaseOutBack,
}

impl Easing {
    /// Evaluate easing function for normalized progress t in [0.0, 1.0].
    pub fn evaluate(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Easing::Linear => t,
            Easing::EaseInQuad => t * t,
            Easing::EaseOutQuad => t * (2.0 - t),
            Easing::EaseInOutQuad => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    -1.0 + (4.0 - 2.0 * t) * t
                }
            }
            Easing::EaseOutCubic => {
                let f = t - 1.0;
                f * f * f + 1.0
            }
            Easing::EaseInOutCubic => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    let f = 2.0 * t - 2.0;
                    0.5 * f * f * f + 1.0
                }
            }
            Easing::EaseOutBack => {
                let c1 = 1.70158;
                let c3 = c1 + 1.0;
                1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
            }
        }
    }
}

#[derive(QueryData)]
#[query_data(mutable)]
pub struct CardHoverLayout {
    interaction: Option<&'static PickingInteraction>,
    lift: &'static mut SpringCardHoverLift,
    transform_opt: Option<&'static mut Transform>,
    node_opt: Option<&'static mut Node>,
}

/// Linear interpolation between two scalar values.
pub fn lerp_f32(start: f32, end: f32, t: f32) -> f32 {
    start + (end - start) * t.clamp(0.0, 1.0)
}

/// Linear interpolation between two Colors in linear sRGB space.
pub fn lerp_color(start: Color, end: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    let s = start.to_srgba();
    let e = end.to_srgba();
    Color::srgba(
        s.red + (e.red - s.red) * t,
        s.green + (e.green - s.green) * t,
        s.blue + (e.blue - s.blue) * t,
        s.alpha + (e.alpha - s.alpha) * t,
    )
}

/// Mass-Spring-Damper harmonic oscillator physics simulator.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spring {
    pub value: f32,
    pub target: f32,
    pub velocity: f32,
    pub stiffness: f32,
    pub damping: f32,
    pub mass: f32,
}

impl Spring {
    pub fn new(initial: f32, stiffness: f32, damping: f32) -> Self {
        Self {
            value: initial,
            target: initial,
            velocity: 0.0,
            stiffness,
            damping,
            mass: 1.0,
        }
    }

    /// Step the simulation forward by `dt` seconds (e.g. 1/60s).
    pub fn step(&mut self, dt: f32) -> f32 {
        let spring_force = -self.stiffness * (self.value - self.target);
        let damping_force = -self.damping * self.velocity;
        let acceleration = (spring_force + damping_force) / self.mass;

        self.velocity += acceleration * dt;
        self.value += self.velocity * dt;
        self.value
    }

    /// Whether the spring has settled within epsilon tolerance.
    pub fn is_settled(&self, epsilon: f32) -> bool {
        (self.value - self.target).abs() < epsilon && self.velocity.abs() < epsilon
    }
}

/// Staggered entry animation scheduler for grids, lists, and modal reveals.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StaggeredEnterAnimation {
    pub stagger_interval_secs: f32,
    pub duration_per_item_secs: f32,
    pub translation_y_px: f32,
}

impl Default for StaggeredEnterAnimation {
    fn default() -> Self {
        Self {
            stagger_interval_secs: 0.04, // 40ms stagger per row
            duration_per_item_secs: 0.25,
            translation_y_px: 20.0,
        }
    }
}

impl StaggeredEnterAnimation {
    pub fn new(stagger_ms: f32, duration_ms: f32, translation_y_px: f32) -> Self {
        Self {
            stagger_interval_secs: stagger_ms / 1000.0,
            duration_per_item_secs: duration_ms / 1000.0,
            translation_y_px,
        }
    }

    /// Compute animation progress `(opacity, translation_y)` for item at index `item_idx` at elapsed time `t`.
    pub fn evaluate_item(&self, item_idx: usize, elapsed_secs: f32) -> (f32, f32) {
        let item_start = item_idx as f32 * self.stagger_interval_secs;
        if elapsed_secs < item_start {
            return (0.0, self.translation_y_px);
        }

        let progress =
            ((elapsed_secs - item_start) / self.duration_per_item_secs.max(1e-4)).clamp(0.0, 1.0);
        let eased = Easing::EaseOutCubic.evaluate(progress);
        let opacity = eased;
        let translation_y = (1.0 - eased) * self.translation_y_px;
        (opacity, translation_y)
    }
}

/// Spring-driven animated value tracker.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct SpringAnimator {
    pub spring: Spring,
    pub is_running: bool,
}

impl SpringAnimator {
    pub fn new(initial: f32, stiffness: f32, damping: f32) -> Self {
        Self {
            spring: Spring::new(initial, stiffness, damping),
            is_running: false,
        }
    }

    pub fn set_target(&mut self, target: f32) {
        self.spring.target = target;
        self.is_running = true;
    }

    pub fn update(&mut self, dt: f32) -> f32 {
        if !self.is_running {
            return self.spring.value;
        }

        let val = self.spring.step(dt);
        if self.spring.is_settled(0.05) {
            self.spring.value = self.spring.target;
            self.spring.velocity = 0.0;
            self.is_running = false;
        }
        val
    }
}

/// Spring-damped scale feedback for buttons and pills upon click / hover.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct SpringButtonPop {
    pub base_scale: f32,
    pub pressed_scale: f32,
    pub hover_scale: f32,
    pub spring: Spring,
}

impl Default for SpringButtonPop {
    fn default() -> Self {
        Self {
            base_scale: 1.0,
            pressed_scale: 0.96,
            hover_scale: 1.0,
            spring: Spring::new(1.0, 320.0, 22.0),
        }
    }
}

impl SpringButtonPop {
    pub fn new(base_scale: f32, pressed_scale: f32, hover_scale: f32) -> Self {
        Self {
            base_scale,
            pressed_scale,
            hover_scale,
            spring: Spring::new(base_scale, 320.0, 22.0),
        }
    }
}

/// Damped spring slide physics for toggle switch knob translation.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct SpringToggleKnob {
    pub off_x: f32,
    pub on_x: f32,
    pub is_on: bool,
    pub spring: Spring,
}

impl SpringToggleKnob {
    pub fn new(off_x: f32, on_x: f32, initial_on: bool) -> Self {
        let initial_x = if initial_on { on_x } else { off_x };
        Self {
            off_x,
            on_x,
            is_on: initial_on,
            spring: Spring::new(initial_x, 260.0, 20.0),
        }
    }

    pub fn set_on(&mut self, is_on: bool) {
        self.is_on = is_on;
        self.spring.target = if is_on { self.on_x } else { self.off_x };
    }
}

/// Elastic route entry transition with vertical dampening and fade.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct SpringRouteTransition {
    pub translation_y: Spring,
    pub is_active: bool,
}

impl Default for SpringRouteTransition {
    fn default() -> Self {
        Self::new(8.0)
    }
}

impl SpringRouteTransition {
    pub fn new(initial_offset_y: f32) -> Self {
        let mut spring = Spring::new(initial_offset_y, 300.0, 24.0);
        spring.target = 0.0;
        Self {
            translation_y: spring,
            is_active: true,
        }
    }
}

/// Staggered card entrance animation for grids and lists.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct SpringStaggeredCard {
    pub index: usize,
    pub elapsed_secs: f32,
    pub stagger: StaggeredEnterAnimation,
    pub is_finished: bool,
}

impl SpringStaggeredCard {
    pub fn new(index: usize, stagger_ms: f32, duration_ms: f32, translation_y_px: f32) -> Self {
        Self {
            index,
            elapsed_secs: 0.0,
            stagger: StaggeredEnterAnimation::new(stagger_ms, duration_ms, translation_y_px),
            is_finished: false,
        }
    }
}

/// Card hover elevation micro-motion lifting card vertically.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct SpringCardHoverLift {
    pub base_y: f32,
    pub lifted_y: f32,
    pub spring: Spring,
}

impl Default for SpringCardHoverLift {
    fn default() -> Self {
        Self {
            base_y: 0.0,
            lifted_y: -4.0,
            spring: Spring::new(0.0, 260.0, 20.0),
        }
    }
}

/// Drive button pressed micro-scaling with spring physics.
pub fn sync_spring_button_pop(
    time: Res<Time>,
    mut buttons: Query<(
        Option<&PickingInteraction>,
        &mut SpringButtonPop,
        Option<&mut Transform>,
    )>,
) {
    let dt = time.delta_secs();
    for (interaction, mut pop, transform_opt) in &mut buttons {
        let target = match interaction {
            Some(PickingInteraction::Pressed) => pop.pressed_scale,
            Some(PickingInteraction::Hovered) => pop.hover_scale,
            _ => pop.base_scale,
        };
        pop.spring.target = target;
        let scale = pop.spring.step(dt);
        if let Some(mut transform) = transform_opt {
            transform.scale = Vec3::splat(scale);
        }
    }
}

/// Drive toggle switch knob slide with damped spring inertia.
pub fn sync_spring_toggle_knob(
    time: Res<Time>,
    mut knobs: Query<(&mut SpringToggleKnob, &mut Node)>,
) {
    let dt = time.delta_secs();
    for (mut knob, mut node) in &mut knobs {
        knob.spring.target = if knob.is_on { knob.on_x } else { knob.off_x };
        let x = knob.spring.step(dt);
        node.left = Val::Px(x);
    }
}

/// Drive route page transition with gentle upward spring settling.
pub fn sync_spring_route_transition(
    time: Res<Time>,
    mut transitions: Query<(
        &mut SpringRouteTransition,
        Option<&mut Transform>,
        Option<&mut Node>,
    )>,
) {
    let dt = time.delta_secs();
    for (mut transition, transform_opt, node_opt) in &mut transitions {
        if !transition.is_active {
            continue;
        }
        let y = transition.translation_y.step(dt);
        if let Some(mut transform) = transform_opt {
            transform.translation.y = y;
        } else if let Some(mut node) = node_opt {
            node.top = Val::Px(y);
        }
        if transition.translation_y.is_settled(0.05) {
            transition.translation_y.value = 0.0;
            transition.is_active = false;
        }
    }
}

/// Drive staggered card list/grid entry animation.
pub fn sync_spring_staggered_cards(
    time: Res<Time>,
    mut cards: Query<(
        &mut SpringStaggeredCard,
        Option<&mut Transform>,
        Option<&mut Node>,
    )>,
) {
    let dt = time.delta_secs();
    for (mut card, transform_opt, node_opt) in &mut cards {
        if card.is_finished {
            continue;
        }
        card.elapsed_secs += dt;
        let total_duration = card.index as f32 * card.stagger.stagger_interval_secs
            + card.stagger.duration_per_item_secs;
        if card.elapsed_secs >= total_duration {
            card.is_finished = true;
            if let Some(mut transform) = transform_opt {
                transform.translation.y = 0.0;
            } else if let Some(mut node) = node_opt {
                node.top = Val::Px(0.0);
            }
        } else {
            let (_opacity, translation_y) =
                card.stagger.evaluate_item(card.index, card.elapsed_secs);
            if let Some(mut transform) = transform_opt {
                transform.translation.y = translation_y;
            } else if let Some(mut node) = node_opt {
                node.top = Val::Px(translation_y);
            }
        }
    }
}

/// Drive card hover lift physics.
pub fn sync_spring_card_hover_lift(time: Res<Time>, mut cards: Query<CardHoverLayout>) {
    let dt = time.delta_secs();
    for CardHoverLayoutItem {
        interaction,
        mut lift,
        transform_opt,
        node_opt,
    } in &mut cards
    {
        let target = match interaction {
            Some(PickingInteraction::Hovered) => lift.lifted_y,
            _ => lift.base_y,
        };
        lift.spring.target = target;
        let y = lift.spring.step(dt);
        if let Some(mut transform) = transform_opt {
            transform.translation.y = y;
        } else if let Some(mut node) = node_opt {
            node.top = Val::Px(y);
        }
    }
}

/// Plugin registering all UI spring physics animation systems.
#[derive(Default)]
pub struct SpringAnimationPlugin;

impl Plugin for SpringAnimationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                sync_spring_button_pop,
                sync_spring_toggle_knob,
                sync_spring_route_transition,
                sync_spring_staggered_cards,
                sync_spring_card_hover_lift,
            ),
        );
    }
}

/// Card micro-3D parallax tilt angle and specular highlight intensity calculator.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CardParallaxTilt {
    pub card_center: Vec2,
    pub card_size: Vec2,
    pub max_tilt_deg: f32,
}

impl CardParallaxTilt {
    pub fn new(center: Vec2, size: Vec2, max_tilt_deg: f32) -> Self {
        Self {
            card_center: center,
            card_size: Vec2::new(size.x.max(1.0), size.y.max(1.0)),
            max_tilt_deg: max_tilt_deg.clamp(1.0, 30.0),
        }
    }

    /// Compute 2D tilt rotation angles (degrees) and specular highlight intensity [0.0..1.0].
    pub fn evaluate(&self, cursor_pos: Vec2) -> (Vec2, f32) {
        let half_size = self.card_size * 0.5;
        let delta = cursor_pos - self.card_center;

        let u = (delta.x / half_size.x).clamp(-1.0, 1.0);
        let v = (delta.y / half_size.y).clamp(-1.0, 1.0);

        // Tilt angles: horizontal cursor delta rotates Y axis, vertical rotates X axis
        let tilt_x_deg = -v * self.max_tilt_deg;
        let tilt_y_deg = u * self.max_tilt_deg;

        // Specular highlight: brighter near center/focus
        let dist = (u * u + v * v).sqrt();
        let highlight = (1.0 - dist / 1.414).clamp(0.0, 1.0);

        (Vec2::new(tilt_x_deg, tilt_y_deg), highlight)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_card_parallax_tilt_evaluation() {
        let center = Vec2::new(200.0, 200.0);
        let size = Vec2::new(300.0, 200.0);
        let tilt_engine = CardParallaxTilt::new(center, size, 10.0);

        // Center cursor -> 0 tilt, max specular highlight
        let (tilt_center, highlight_center) = tilt_engine.evaluate(center);
        assert_eq!(tilt_center, Vec2::ZERO);
        assert!((highlight_center - 1.0).abs() < 1e-4);

        // Top-right corner cursor (350, 100) -> u=1, v=-1
        let (tilt_corner, highlight_corner) = tilt_engine.evaluate(Vec2::new(350.0, 100.0));
        assert_eq!(tilt_corner.x, 10.0); // -(-1) * 10
        assert_eq!(tilt_corner.y, 10.0); // 1 * 10
        assert!(highlight_corner < 0.1);
    }

    #[test]
    fn test_spring_animator_step_and_settling() {
        let mut animator = SpringAnimator::new(0.0, 300.0, 20.0);
        animator.set_target(10.0);
        assert!(animator.is_running);

        for _ in 0..100 {
            animator.update(0.016);
        }
        assert!(!animator.is_running);
        assert!((animator.spring.value - 10.0).abs() < 0.05);
    }

    #[test]
    fn test_spring_toggle_knob_slide() {
        let mut knob = SpringToggleKnob::new(2.0, 22.0, false);
        assert_eq!(knob.spring.value, 2.0);

        knob.set_on(true);
        assert_eq!(knob.spring.target, 22.0);

        for _ in 0..60 {
            knob.spring.step(0.016);
        }
        assert!((knob.spring.value - 22.0).abs() < 0.5);
    }

    #[test]
    fn test_spring_route_transition_physics() {
        let mut transition = SpringRouteTransition::new(8.0);
        assert!(transition.is_active);
        assert_eq!(transition.translation_y.value, 8.0);
        assert_eq!(transition.translation_y.target, 0.0);

        for _ in 0..60 {
            transition.translation_y.step(0.016);
        }
        assert!(transition.translation_y.value < 0.5);
    }

    #[test]
    fn test_spring_staggered_card_evaluation() {
        let card = SpringStaggeredCard::new(2, 25.0, 200.0, 16.0);
        assert!(!card.is_finished);

        // Before start of item 2 (2 * 25ms = 50ms = 0.05s)
        let (opacity_0, y_0) = card.stagger.evaluate_item(card.index, 0.02);
        assert_eq!(opacity_0, 0.0);
        assert_eq!(y_0, 16.0);

        // After completion
        let (opacity_done, y_done) = card.stagger.evaluate_item(card.index, 0.5);
        assert!((opacity_done - 1.0).abs() < 1e-4);
        assert!(y_done.abs() < 1e-4);
    }
}
