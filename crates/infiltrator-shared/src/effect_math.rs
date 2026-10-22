//! The shared effect mathematics (EFFECTS-004 #14~#17).
//!
//! Four algorithms the whole product animates and draws with:
//!
//! * the Oklab perceptual colour space and its interpolation (#14);
//! * the cubic Bézier curve evaluator (#15);
//! * the fourth-order Runge-Kutta spring integrator (#16);
//! * the analytic signed distance fields of a rounded rectangle and of a
//!   pill (#17).
//!
//! The module owns the *algorithms*, never the product's numbers: the spring
//! coefficients come from the motion contract, the colours from the palette
//! contract. A surface that animates calls these functions instead of
//! re-deriving an easing curve of its own.
//!
//! Every algorithm answers with a number a surface can draw with, and every
//! number is defined on the whole input range: a frame that arrives late, a
//! progress that is out of range and a distance that is not finite all have
//! one documented answer instead of an accident.

/// One point in a plane.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    /// The horizontal coordinate.
    pub x: f32,
    /// The vertical coordinate.
    pub y: f32,
}

impl Point {
    /// One point.
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// The length of the vector from the origin to this point.
    pub fn length(self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    /// This point moved by `other`.
    pub const fn add(self, other: Point) -> Self {
        Self {
            x: self.x + other.x,
            y: self.y + other.y,
        }
    }

    /// This point moved back by `other`.
    pub const fn sub(self, other: Point) -> Self {
        Self {
            x: self.x - other.x,
            y: self.y - other.y,
        }
    }

    /// This point moved by `factor` times `other`.
    pub const fn scale(self, factor: f32) -> Self {
        Self {
            x: self.x * factor,
            y: self.y * factor,
        }
    }
}

/// One colour in the sRGB space, with channels in 0.0..=1.0.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgb {
    /// The red channel.
    pub r: f32,
    /// The green channel.
    pub g: f32,
    /// The blue channel.
    pub b: f32,
}

impl Rgb {
    /// One colour.
    pub const fn new(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b }
    }
}

/// One colour in the Oklab space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Oklab {
    /// The perceptual lightness.
    pub l: f32,
    /// The green-red axis.
    pub a: f32,
    /// The blue-yellow axis.
    pub b: f32,
}

/// One colour in the Oklch space: the Oklab space written as a cylinder.
///
/// The hue is an angle in degrees, measured counter-clockwise from the
/// positive a-axis. A grey has no hue of its own, so its hue is zero.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Oklch {
    /// The perceptual lightness.
    pub l: f32,
    /// The chroma: the distance from the neutral axis.
    pub c: f32,
    /// The hue, in degrees.
    pub h: f32,
}

/// The sRGB electro-optical transfer function (one channel).
pub fn srgb_to_linear(channel: f32) -> f32 {
    let value = channel.clamp(0.0, 1.0);
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

/// The inverse sRGB electro-optical transfer function (one channel).
pub fn linear_to_srgb(channel: f32) -> f32 {
    let value = channel.clamp(0.0, 1.0);
    if value <= 0.0031308 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    }
}

/// The real cube root of a number, defined for negative inputs as well.
fn cbrt(value: f32) -> f32 {
    if value < 0.0 {
        -(-value).powf(1.0 / 3.0)
    } else {
        value.powf(1.0 / 3.0)
    }
}

/// Convert one sRGB colour into the Oklab space.
pub fn rgb_to_oklab(color: Rgb) -> Oklab {
    let red = srgb_to_linear(color.r);
    let green = srgb_to_linear(color.g);
    let blue = srgb_to_linear(color.b);

    let long = 0.4122215 * red + 0.5363325 * green + 0.051446 * blue;
    let medium = 0.2119035 * red + 0.6806995 * green + 0.107397 * blue;
    let short = 0.0883025 * red + 0.2817188 * green + 0.6299787 * blue;

    let long_root = cbrt(long);
    let medium_root = cbrt(medium);
    let short_root = cbrt(short);

    Oklab {
        l: 0.2104543 * long_root + 0.7936178 * medium_root - 0.004072 * short_root,
        a: 1.9779985 * long_root - 2.4285922 * medium_root + 0.4505937 * short_root,
        b: 0.025904 * long_root + 0.7827718 * medium_root - 0.8086758 * short_root,
    }
}

/// Convert one Oklab colour back into the sRGB space.
///
/// A colour outside the sRGB gamut is clamped per channel, which is what a
/// renderer has to do with it anyway.
pub fn oklab_to_rgb(color: Oklab) -> Rgb {
    let long_root = color.l + 0.3963378 * color.a + 0.2158038 * color.b;
    let medium_root = color.l - 0.1055613 * color.a - 0.0638542 * color.b;
    let short_root = color.l - 0.0894842 * color.a - 1.2914855 * color.b;

    let long = long_root * long_root * long_root;
    let medium = medium_root * medium_root * medium_root;
    let short = short_root * short_root * short_root;

    Rgb {
        r: linear_to_srgb(4.0767417 * long - 3.3077116 * medium + 0.2309699 * short),
        g: linear_to_srgb(-1.268438 * long + 2.6097574 * medium - 0.3413194 * short),
        b: linear_to_srgb(-0.0041961 * long - 0.7034186 * medium + 1.7076147 * short),
    }
}

/// Convert one Oklab colour into the Oklch space.
pub fn oklab_to_oklch(color: Oklab) -> Oklch {
    let chroma = (color.a * color.a + color.b * color.b).sqrt();
    let mut hue = color.b.atan2(color.a).to_degrees();
    if hue < 0.0 {
        hue += 360.0;
    }
    Oklch {
        l: color.l,
        c: chroma,
        h: hue,
    }
}

/// Convert one Oklch colour back into the Oklab space.
pub fn oklch_to_oklab(color: Oklch) -> Oklab {
    let (sinus, cosine) = color.h.to_radians().sin_cos();
    Oklab {
        l: color.l,
        a: color.c * cosine,
        b: color.c * sinus,
    }
}

/// The perceptual lightness of one sRGB colour, in 0.0..=1.0.
pub fn perceptual_lightness(color: Rgb) -> f32 {
    rgb_to_oklab(color).l
}

/// The perceptual distance between two colours.
///
/// The distance is measured in the Oklab space, where one unit is roughly
/// one just-noticeable difference of lightness.
pub fn oklab_distance(from: Rgb, to: Rgb) -> f32 {
    let start = rgb_to_oklab(from);
    let end = rgb_to_oklab(to);
    let lightness = end.l - start.l;
    let green_red = end.a - start.a;
    let blue_yellow = end.b - start.b;
    (lightness * lightness + green_red * green_red + blue_yellow * blue_yellow).sqrt()
}

/// Mix two colours through the Oklab space.
///
/// A mix of two colours is *perceptually* even: the halfway colour of a
/// dark-to-light ramp keeps the same apparent distance from both ends, which
/// a naive sRGB mix does not.
pub fn mix_oklab(from: Rgb, to: Rgb, amount: f32) -> Rgb {
    let t = if amount.is_finite() {
        amount.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let start = rgb_to_oklab(from);
    let end = rgb_to_oklab(to);
    oklab_to_rgb(Oklab {
        l: start.l + (end.l - start.l) * t,
        a: start.a + (end.a - start.a) * t,
        b: start.b + (end.b - start.b) * t,
    })
}

/// Mix two colours through the Oklch space, keeping the hue on the short arc.
///
/// The mix walks the hue along the shorter arc between the two ends, so a
/// transition between two hues never passes through an unrelated one. The
/// lightness and the chroma move linearly, the hue along the arc.
pub fn mix_oklch(from: Rgb, to: Rgb, amount: f32) -> Rgb {
    let t = if amount.is_finite() {
        amount.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let start = oklab_to_oklch(rgb_to_oklab(from));
    let end = oklab_to_oklch(rgb_to_oklab(to));
    let delta = end.h - start.h;
    let delta = if delta > 180.0 {
        delta - 360.0
    } else if delta < -180.0 {
        delta + 360.0
    } else {
        delta
    };
    oklab_to_rgb(oklch_to_oklab(Oklch {
        l: start.l + (end.l - start.l) * t,
        c: start.c + (end.c - start.c) * t,
        h: start.h + delta * t,
    }))
}

/// Evaluate one cubic Bézier segment in a single dimension.
pub fn cubic_bezier(t: f32, p0: f32, p1: f32, p2: f32, p3: f32) -> f32 {
    let u = if t.is_finite() {
        t.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let inverse = 1.0 - u;
    inverse * inverse * inverse * p0
        + 3.0 * inverse * inverse * u * p1
        + 3.0 * inverse * u * u * p2
        + u * u * u * p3
}

/// Evaluate one cubic Bézier segment in the plane.
pub fn cubic_bezier_point(control: [Point; 4], t: f32) -> Point {
    Point {
        x: cubic_bezier(t, control[0].x, control[1].x, control[2].x, control[3].x),
        y: cubic_bezier(t, control[0].y, control[1].y, control[2].y, control[3].y),
    }
}

/// The tangent of one cubic Bézier segment at `t`.
pub fn cubic_bezier_tangent(control: [Point; 4], t: f32) -> Point {
    let u = if t.is_finite() {
        t.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let inverse = 1.0 - u;
    Point {
        x: 3.0
            * (inverse * inverse * (control[1].x - control[0].x)
                + 2.0 * inverse * u * (control[2].x - control[1].x)
                + u * u * (control[3].x - control[2].x)),
        y: 3.0
            * (inverse * inverse * (control[1].y - control[0].y)
                + 2.0 * inverse * u * (control[2].y - control[1].y)
                + u * u * (control[3].y - control[2].y)),
    }
}

/// A cubic Bézier easing curve.
///
/// The curve runs from (0, 0) to (1, 1) with the two control points of the
/// specification in between, so it is the curve a browser draws for
/// `cubic-bezier(x1, y1, x2, y2)`. Evaluating it at a progress means
/// solving the curve for the parameter that puts it at that progress, which
/// is why the evaluation carries its own root finder.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BezierEasing {
    /// The x coordinate of the first control point.
    pub x1: f32,
    /// The y coordinate of the first control point.
    pub y1: f32,
    /// The x coordinate of the second control point.
    pub x2: f32,
    /// The y coordinate of the second control point.
    pub y2: f32,
}

impl BezierEasing {
    /// One easing curve.
    pub const fn new(x1: f32, y1: f32, x2: f32, y2: f32) -> Self {
        Self { x1, y1, x2, y2 }
    }

    /// The four control points of the curve.
    pub const fn control_points(&self) -> [Point; 4] {
        [
            Point::new(0.0, 0.0),
            Point::new(self.x1, self.y1),
            Point::new(self.x2, self.y2),
            Point::new(1.0, 1.0),
        ]
    }

    /// Evaluate the curve at a progress in 0.0..=1.0.
    ///
    /// The parameter of the curve is solved with a Newton-Raphson step and a
    /// bisection fallback, so the result is the y coordinate of the curve at
    /// the requested x coordinate.
    pub fn evaluate(&self, progress: f32) -> f32 {
        let target = if progress.is_finite() {
            progress.clamp(0.0, 1.0)
        } else {
            0.0
        };
        if target <= 0.0 {
            return 0.0;
        }
        if target >= 1.0 {
            return 1.0;
        }
        let control = self.control_points();
        let mut parameter = target;
        for _ in 0..8 {
            let error = cubic_bezier(
                parameter,
                control[0].x,
                control[1].x,
                control[2].x,
                control[3].x,
            ) - target;
            if error.abs() < 1e-6 {
                break;
            }
            let inverse = 1.0 - parameter;
            let slope = 3.0 * inverse * inverse * control[1].x
                + 6.0 * inverse * parameter * (control[2].x - control[1].x)
                + 3.0 * parameter * parameter * (1.0 - control[2].x);
            if slope.abs() < 1e-6 {
                break;
            }
            parameter = (parameter - error / slope).clamp(0.0, 1.0);
        }
        let estimate = cubic_bezier(
            parameter,
            control[0].x,
            control[1].x,
            control[2].x,
            control[3].x,
        );
        if (estimate - target).abs() > 1e-5 {
            parameter = solve_bezier_parameter(target, &control);
        }
        cubic_bezier(
            parameter,
            control[0].y,
            control[1].y,
            control[2].y,
            control[3].y,
        )
    }
}

/// Solve the parameter of one Bézier curve that puts it at `x`.
///
/// The bisection assumes that the x coordinate of the curve grows with the
/// parameter, which holds for every easing curve the product uses.
fn solve_bezier_parameter(target: f32, control: &[Point; 4]) -> f32 {
    let mut low = 0.0_f32;
    let mut high = 1.0_f32;
    let mut middle = target;
    for _ in 0..32 {
        middle = (low + high) * 0.5;
        let value = cubic_bezier(
            middle,
            control[0].x,
            control[1].x,
            control[2].x,
            control[3].x,
        );
        if value < target {
            low = middle;
        } else {
            high = middle;
        }
    }
    middle
}

/// The state of one spring animation.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SpringState {
    /// The current value of the animated quantity.
    pub value: f32,
    /// The current rate of change of the value, per second.
    pub velocity: f32,
}

/// The largest integration step the spring integrator takes.
pub const MAX_SPRING_STEP_SECONDS: f32 = 1.0 / 240.0;

/// The largest number of integration steps one call may take.
pub const MAX_SPRING_STEPS: usize = 240;

/// The tolerance of the damping regime comparison.
const DAMPING_RATIO_TOLERANCE: f32 = 1e-6;

/// The derivative of the spring state.
fn spring_derivative(state: SpringState, target: f32, stiffness: f32, damping: f32) -> SpringState {
    SpringState {
        value: state.velocity,
        velocity: -stiffness * (state.value - target) - damping * state.velocity,
    }
}

/// One Runge-Kutta step of the spring state.
fn rk4_step(state: SpringState, target: f32, stiffness: f32, damping: f32, dt: f32) -> SpringState {
    let first = spring_derivative(state, target, stiffness, damping);
    let second = spring_derivative(
        SpringState {
            value: state.value + first.value * dt * 0.5,
            velocity: state.velocity + first.velocity * dt * 0.5,
        },
        target,
        stiffness,
        damping,
    );
    let third = spring_derivative(
        SpringState {
            value: state.value + second.value * dt * 0.5,
            velocity: state.velocity + second.velocity * dt * 0.5,
        },
        target,
        stiffness,
        damping,
    );
    let fourth = spring_derivative(
        SpringState {
            value: state.value + third.value * dt,
            velocity: state.velocity + third.velocity * dt,
        },
        target,
        stiffness,
        damping,
    );
    SpringState {
        value: state.value
            + dt / 6.0 * (first.value + 2.0 * second.value + 2.0 * third.value + fourth.value),
        velocity: state.velocity
            + dt / 6.0
                * (first.velocity + 2.0 * second.velocity + 2.0 * third.velocity + fourth.velocity),
    }
}

/// Advance one spring animation by `dt_seconds`.
///
/// The integration is sub-stepped, so a long frame cannot make the spring
/// diverge: the step size stays at or below [`MAX_SPRING_STEP_SECONDS`]. A
/// call takes at most [`MAX_SPRING_STEPS`] steps, so it simulates at most one
/// second of motion; a longer frame is shortened instead of being integrated
/// with a coarse step, which would let the spring grow without bound.
/// The coefficients are the product's own; they come from the motion
/// contract, not from this module.
pub fn spring_step(
    state: SpringState,
    target: f32,
    stiffness: f32,
    damping: f32,
    dt_seconds: f32,
) -> SpringState {
    if !dt_seconds.is_finite() || dt_seconds <= 0.0 {
        return state;
    }
    let steps = (dt_seconds / MAX_SPRING_STEP_SECONDS)
        .ceil()
        .max(1.0)
        .min(MAX_SPRING_STEPS as f32) as usize;
    let dt = (dt_seconds / steps as f32).min(MAX_SPRING_STEP_SECONDS);
    let mut current = state;
    for _ in 0..steps {
        current = rk4_step(current, target, stiffness, damping, dt);
    }
    current
}

/// The mechanical energy of one spring state.
pub fn spring_energy(state: SpringState, target: f32, stiffness: f32) -> f32 {
    let displacement = state.value - target;
    0.5 * state.velocity * state.velocity + 0.5 * stiffness * displacement * displacement
}

/// Whether one spring animation has settled at its target.
pub fn spring_settled(state: SpringState, target: f32, epsilon: f32) -> bool {
    (state.value - target).abs() <= epsilon && state.velocity.abs() <= epsilon
}

/// How a spring behaves around its target.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DampingRegime {
    /// The spring swings past its target before it settles.
    Underdamped,
    /// The spring reaches its target as fast as it can without swinging.
    CriticallyDamped,
    /// The spring creeps towards its target without swinging.
    Overdamped,
}

/// The damping regime of a spring with the given coefficients.
///
/// The critical damping of a spring is `2 * sqrt(stiffness)`: a spring below
/// it swings, a spring above it creeps. A spring without a restoring force
/// cannot swing at all and is reported as over-damped; a damping that cannot
/// be compared is reported as critically damped.
pub fn damping_regime(stiffness: f32, damping: f32) -> DampingRegime {
    if !stiffness.is_finite() || stiffness <= 0.0 {
        return DampingRegime::Overdamped;
    }
    let critical = 2.0 * stiffness.sqrt();
    let ratio = damping / critical;
    if ratio < 1.0 - DAMPING_RATIO_TOLERANCE {
        DampingRegime::Underdamped
    } else if ratio > 1.0 + DAMPING_RATIO_TOLERANCE {
        DampingRegime::Overdamped
    } else {
        DampingRegime::CriticallyDamped
    }
}

/// The signed distance from a point to a rounded rectangle.
///
/// The rectangle is centred on the origin; the distance is negative inside
/// the shape and positive outside, and it is exact: the value at the outline
/// is zero and its gradient has unit length.
pub fn rounded_rect_sdf(point: Point, half_size: Point, radius: f32) -> f32 {
    let corner = radius.max(0.0).min(half_size.x).min(half_size.y);
    let dx = point.x.abs() - half_size.x + corner;
    let dy = point.y.abs() - half_size.y + corner;
    let outside = (dx.max(0.0) * dx.max(0.0) + dy.max(0.0) * dy.max(0.0)).sqrt();
    let inside = dx.max(dy).min(0.0);
    outside + inside - corner
}

/// The signed distance to a band around the outline of a rounded rectangle.
///
/// The band holds every point whose distance to the outline is at most half
/// the thickness, so the value is negative inside the band and zero on its
/// two edges.
pub fn rounded_rect_ring_sdf(point: Point, half_size: Point, radius: f32, thickness: f32) -> f32 {
    rounded_rect_sdf(point, half_size, radius).abs() - thickness.max(0.0) * 0.5
}

/// The outward direction of the distance field of a rounded rectangle.
///
/// The vector has unit length. On the medial axis of the shape the distance
/// field has no direction of its own, and the nearest valid one is returned.
pub fn rounded_rect_normal(point: Point, half_size: Point, radius: f32) -> Point {
    let corner = radius.max(0.0).min(half_size.x).min(half_size.y);
    let dx = point.x.abs() - half_size.x + corner;
    let dy = point.y.abs() - half_size.y + corner;
    let sign_x = if point.x < 0.0 { -1.0 } else { 1.0 };
    let sign_y = if point.y < 0.0 { -1.0 } else { 1.0 };
    if dx > 0.0 && dy > 0.0 {
        let length = (dx * dx + dy * dy).sqrt();
        Point::new(sign_x * dx / length, sign_y * dy / length)
    } else if dx > dy {
        Point::new(sign_x, 0.0)
    } else {
        Point::new(0.0, sign_y)
    }
}

/// The signed distance from a point to a capsule.
///
/// A capsule is a segment with a round cap of the given radius at each end;
/// the distance is negative inside the shape and positive outside.
pub fn capsule_sdf(point: Point, from: Point, to: Point, radius: f32) -> f32 {
    let axis = to.sub(from);
    let length_squared = axis.x * axis.x + axis.y * axis.y;
    let relative = point.sub(from);
    let projection = if length_squared <= f32::EPSILON {
        0.0
    } else {
        ((relative.x * axis.x + relative.y * axis.y) / length_squared).clamp(0.0, 1.0)
    };
    let closest = from.add(axis.scale(projection));
    point.sub(closest).length() - radius.max(0.0)
}

/// The outward direction of the distance field of a capsule.
///
/// The radius does not change the direction of the field, only the distance.
pub fn capsule_normal(point: Point, from: Point, to: Point) -> Point {
    let axis = to.sub(from);
    let length_squared = axis.x * axis.x + axis.y * axis.y;
    let relative = point.sub(from);
    let projection = if length_squared <= f32::EPSILON {
        0.0
    } else {
        ((relative.x * axis.x + relative.y * axis.y) / length_squared).clamp(0.0, 1.0)
    };
    let closest = from.add(axis.scale(projection));
    let delta = point.sub(closest);
    let length = delta.length();
    if length <= f32::EPSILON {
        let perpendicular = Point::new(-axis.y, axis.x);
        let perpendicular_length = perpendicular.length();
        if perpendicular_length <= f32::EPSILON {
            Point::new(0.0, 1.0)
        } else {
            perpendicular.scale(1.0 / perpendicular_length)
        }
    } else {
        delta.scale(1.0 / length)
    }
}

/// The signed distance from a point to a horizontal pill (a capsule).
///
/// The capsule is centred on the origin; its straight section is
/// `2 * half_length` long and its caps have the given radius.
pub fn pill_sdf(point: Point, half_length: f32, radius: f32) -> f32 {
    capsule_sdf(
        point,
        Point::new(-half_length, 0.0),
        Point::new(half_length, 0.0),
        radius,
    )
}

/// The outward direction of the distance field of a horizontal pill.
pub fn pill_normal(point: Point, half_length: f32) -> Point {
    capsule_normal(
        point,
        Point::new(-half_length, 0.0),
        Point::new(half_length, 0.0),
    )
}

/// The anti-aliased coverage of a shape at one distance.
///
/// The coverage is 1 inside the shape, 0 outside it, and it falls smoothly
/// over a band of the given softness around the outline. A softness of zero
/// draws a hard edge.
pub fn sdf_coverage(distance: f32, softness: f32) -> f32 {
    if !distance.is_finite() {
        return 0.0;
    }
    let width = softness.max(0.0);
    if width <= f32::EPSILON {
        return if distance <= 0.0 { 1.0 } else { 0.0 };
    }
    let t = ((distance + width) / (2.0 * width)).clamp(0.0, 1.0);
    1.0 - t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
#[path = "effect_math_tests.rs"]
mod tests;
