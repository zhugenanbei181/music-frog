//! Animation easing curves, spring physics simulation, and color interpolation.

use bevy::color::Color;

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
#[derive(Clone, Copy, Debug, PartialEq)]
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

use bevy::math::Vec2;

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
}
