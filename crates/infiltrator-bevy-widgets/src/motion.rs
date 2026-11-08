//! Animation easing curves, spring physics simulation, and color interpolation.

use crate::palette::rem;
use bevy::app::{App, Plugin, Update};
use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, Res};
use bevy::time::Time;
use bevy::transform::components::Transform;
use bevy::ui::Node;
use zhuge_ui_core::easing::lerp;

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

/// Host preference for non-essential UI motion.
///
/// This is the widgets layer's single reduce-motion truth: scroll coasting,
/// control glides and other decorative animation read the same resource, and
/// the shell maps its typed host accessibility intent onto it.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum UiMotionPreference {
    /// Apply bounded decorative motion.
    #[default]
    Full,
    /// Snap decorative motion to its rest state without travelling.
    Reduced,
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

/// Linear interpolation between two scalar values.
pub fn lerp_f32(start: f32, end: f32, t: f32) -> f32 {
    lerp(start, end, t.clamp(0.0, 1.0))
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

/// Longest slice integrated in one substep. The stiffest spring in the
/// workspace is 320 N/m, whose semi-implicit Euler stability limit
/// (`2 / sqrt(stiffness)` ≈ 0.112 s) leaves a wide margin below this.
const SPRING_MAX_SUBSTEP_SECS: f32 = 1.0 / 60.0;
/// Simulated-time budget for a single call: past a half second of stalled
/// frame the animation keeps catching up over the following frames instead
/// of integrating an unbounded delta at once.
const SPRING_MAX_SIMULATED_SECS: f32 = 0.5;

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
        if !dt.is_finite() || dt <= 0.0 {
            return self.value;
        }
        // Semi-implicit Euler is only stable for steps below roughly
        // `2 / sqrt(stiffness)` seconds. One stalled frame (compositor stall,
        // window drag, capture screenshot) delivers a much longer delta, so
        // the excess is integrated in bounded substeps instead of amplifying
        // the oscillator until the value overflows into NaN.
        let mut remaining = dt.min(SPRING_MAX_SIMULATED_SECS);
        while remaining > 0.0 {
            let substep = remaining.min(SPRING_MAX_SUBSTEP_SECS);
            let spring_force = -self.stiffness * (self.value - self.target);
            let damping_force = -self.damping * self.velocity;
            let acceleration = (spring_force + damping_force) / self.mass;
            self.velocity += acceleration * substep;
            self.value += self.velocity * substep;
            remaining -= substep;
        }
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
            node.top = rem(y);
        }
        if transition.translation_y.is_settled(0.05) {
            transition.translation_y.value = 0.0;
            transition.is_active = false;
        }
    }
}

/// Plugin registering all UI spring physics animation systems.
#[derive(Default)]
pub struct SpringAnimationPlugin;

impl Plugin for SpringAnimationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, sync_spring_route_transition);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_spring_step_survives_stalled_frame() {
        // One second-long delta used to amplify the oscillator until the
        // value overflowed into NaN and the layout died on a clamp panic.
        let mut spring = Spring::new(8.0, 300.0, 24.0);
        spring.target = 0.0;
        for _ in 0..12 {
            spring.step(1.0);
            assert!(spring.value.is_finite());
            assert!(spring.value.abs() < 16.0);
        }
        assert!(spring.is_settled(0.05));
    }
}
