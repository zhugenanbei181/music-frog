//! The proofs of the shared effect mathematics (EFFECTS-004 #14~#17).
//!
//! Every algorithm of the module is proven by the behaviour the product
//! depends on: a colour ramp that moves evenly, a spring whose energy only
//! ever fades, and distance fields that are exact on the outline and
//! negative inside the shape.

use super::*;

fn assert_close(actual: f32, expected: f32, epsilon: f32) {
    assert!(
        (actual - expected).abs() <= epsilon,
        "expected {expected}, measured {actual}"
    );
}

/// The naive mix of two colours, channel by channel.
fn naive_mix(from: Rgb, to: Rgb, amount: f32) -> Rgb {
    Rgb::new(
        from.r + (to.r - from.r) * amount,
        from.g + (to.g - from.g) * amount,
        from.b + (to.b - from.b) * amount,
    )
}

/// The hue of one colour, in degrees.
fn hue_of(color: Rgb) -> f32 {
    oklab_to_oklch(rgb_to_oklab(color)).h
}

/// The distance between two hues, in degrees, along the short arc.
fn hue_distance(from: f32, to: f32) -> f32 {
    let difference = (from - to).abs() % 360.0;
    if difference > 180.0 {
        360.0 - difference
    } else {
        difference
    }
}

#[test]
fn the_oklab_round_trip_is_lossless() {
    for color in [
        Rgb::new(0.0, 0.0, 0.0),
        Rgb::new(1.0, 1.0, 1.0),
        Rgb::new(0.12, 0.56, 0.96),
        Rgb::new(0.93, 0.95, 0.94),
        Rgb::new(0.24, 0.78, 0.44),
    ] {
        let back = oklab_to_rgb(rgb_to_oklab(color));
        assert_close(back.r, color.r, 1e-4);
        assert_close(back.g, color.g, 1e-4);
        assert_close(back.b, color.b, 1e-4);
    }
}

#[test]
fn the_oklab_mix_is_perceptually_even() {
    let dark = Rgb::new(0.05, 0.05, 0.05);
    let light = Rgb::new(0.95, 0.95, 0.95);
    let start = perceptual_lightness(dark);
    let end = perceptual_lightness(light);
    let middle = perceptual_lightness(mix_oklab(dark, light, 0.5));
    assert!(
        middle > start && middle < end,
        "the halfway colour must sit between the two ends"
    );
    // The perceptual distance to the two ends is symmetric.
    assert_close(middle - start, end - middle, 0.01);
    // The endpoints are exact.
    assert_close(mix_oklab(dark, light, 0.0).r, dark.r, 1e-4);
    assert_close(mix_oklab(dark, light, 1.0).r, light.r, 1e-4);
}

#[test]
fn the_oklab_ramp_moves_the_lightness_evenly() {
    let dark = Rgb::new(0.05, 0.05, 0.05);
    let light = Rgb::new(0.95, 0.95, 0.95);
    let start = perceptual_lightness(dark);
    let end = perceptual_lightness(light);
    let mut previous = start;
    for step in 1..=20 {
        let amount = step as f32 / 20.0;
        let lightness = perceptual_lightness(mix_oklab(dark, light, amount));
        assert!(
            lightness > previous,
            "the lightness must grow with the amount, measured {lightness} after {previous}"
        );
        assert_close(lightness, start + (end - start) * amount, 1e-4);
        previous = lightness;
    }
    // The naive mix of the same two colours is not even: its halfway colour
    // is perceptually brighter than the halfway of the ramp.
    let naive_middle = perceptual_lightness(naive_mix(dark, light, 0.5));
    assert!(
        (naive_middle - (start + end) * 0.5).abs() > 0.01,
        "the naive mix must not be perceptually even, measured {naive_middle}"
    );
}

#[test]
fn the_oklch_round_trip_is_lossless() {
    for color in [
        Rgb::new(0.12, 0.56, 0.96),
        Rgb::new(0.93, 0.95, 0.94),
        Rgb::new(0.24, 0.78, 0.44),
        Rgb::new(0.5, 0.5, 0.5),
    ] {
        let cylindrical = oklab_to_oklch(rgb_to_oklab(color));
        let back = oklab_to_rgb(oklch_to_oklab(cylindrical));
        assert_close(back.r, color.r, 1e-4);
        assert_close(back.g, color.g, 1e-4);
        assert_close(back.b, color.b, 1e-4);
    }
}

#[test]
fn the_hue_mix_walks_the_short_arc() {
    let red = Rgb::new(1.0, 0.0, 0.0);
    let blue = Rgb::new(0.0, 0.0, 1.0);
    let start_hue = hue_of(red);
    let end_hue = hue_of(blue);
    let middle = mix_oklch(red, blue, 0.5);
    let middle_hue = hue_of(middle);
    // The halfway colour sits on the short arc between the two hues, so it
    // is close to both ends and never on the long way round.
    assert!(
        hue_distance(middle_hue, start_hue) < 90.0,
        "the halfway hue must stay near the first end, measured {middle_hue}"
    );
    assert!(
        hue_distance(middle_hue, end_hue) < 90.0,
        "the halfway hue must stay near the second end, measured {middle_hue}"
    );
    // The endpoints are exact.
    let from = mix_oklch(red, blue, 0.0);
    assert_close(from.r, red.r, 1e-3);
    assert_close(from.b, red.b, 1e-3);
    let to = mix_oklch(red, blue, 1.0);
    assert_close(to.r, blue.r, 1e-3);
    assert_close(to.b, blue.b, 1e-3);
    // Two colours of one hue keep that hue through the whole mix.
    let dim_red = Rgb::new(0.5, 0.0, 0.0);
    let hue = hue_of(dim_red);
    for step in 0..=10 {
        let amount = step as f32 / 10.0;
        let mixed = mix_oklch(dim_red, red, amount);
        assert!(
            hue_distance(hue_of(mixed), hue) < 0.5,
            "the hue must not move, measured {}",
            hue_of(mixed)
        );
    }
}

#[test]
fn the_perceptual_distance_is_symmetric() {
    let cold = Rgb::new(0.1, 0.2, 0.3);
    let warm = Rgb::new(0.8, 0.7, 0.6);
    assert_close(oklab_distance(cold, warm), oklab_distance(warm, cold), 1e-6);
    assert!(oklab_distance(cold, cold) < 1e-6);
    assert!(oklab_distance(cold, warm) > 0.1);
}

#[test]
fn the_bezier_curve_hits_its_endpoints_and_stays_in_the_hull() {
    let control = [
        Point::new(0.0, 0.0),
        Point::new(0.0, 10.0),
        Point::new(10.0, 10.0),
        Point::new(10.0, 0.0),
    ];
    let start = cubic_bezier_point(control, 0.0);
    let end = cubic_bezier_point(control, 1.0);
    assert_close(start.x, control[0].x, 1e-5);
    assert_close(start.y, control[0].y, 1e-5);
    assert_close(end.x, control[3].x, 1e-5);
    assert_close(end.y, control[3].y, 1e-5);
    for step in 0..=20 {
        let t = step as f32 / 20.0;
        let point = cubic_bezier_point(control, t);
        assert!(point.x >= -1e-4 && point.x <= 10.0 + 1e-4);
        assert!(point.y >= -1e-4 && point.y <= 10.0 + 1e-4);
    }
    // The tangent at the start points along the first control segment.
    let tangent = cubic_bezier_tangent(control, 0.0);
    assert_close(tangent.x, 0.0, 1e-4);
    assert!(tangent.y > 0.0);
}

#[test]
fn the_easing_curve_hits_its_ends() {
    let easing = BezierEasing::new(0.25, 0.1, 0.25, 1.0);
    assert_close(easing.evaluate(0.0), 0.0, 1e-6);
    assert_close(easing.evaluate(1.0), 1.0, 1e-6);
    assert_close(easing.evaluate(-1.0), 0.0, 1e-6);
    assert_close(easing.evaluate(2.0), 1.0, 1e-6);
    assert_close(easing.evaluate(f32::NAN), 0.0, 1e-6);
}

#[test]
fn the_linear_easing_is_the_identity() {
    let easing = BezierEasing::new(1.0 / 3.0, 1.0 / 3.0, 2.0 / 3.0, 2.0 / 3.0);
    for step in 0..=20 {
        let amount = step as f32 / 20.0;
        assert_close(easing.evaluate(amount), amount, 1e-4);
    }
}

#[test]
fn the_easing_curve_follows_its_specification() {
    let easing = BezierEasing::new(0.42, 0.0, 0.58, 1.0);
    let control = easing.control_points();
    let mut previous = 0.0;
    for step in 0..=100 {
        let amount = step as f32 / 100.0;
        let value = easing.evaluate(amount);
        assert!(
            value >= previous - 1e-6,
            "the easing must not go backwards, measured {value} after {previous}"
        );
        previous = value;
    }
    // The evaluation is the y coordinate of the curve at the solved
    // parameter: a fine scan of the curve finds the same y.
    for step in 1..=9 {
        let amount = step as f32 / 10.0;
        let mut best = amount;
        let mut best_error = (cubic_bezier(
            amount,
            control[0].x,
            control[1].x,
            control[2].x,
            control[3].x,
        ) - amount)
            .abs();
        for scan in 0..=2000 {
            let parameter = scan as f32 / 2000.0;
            let error = (cubic_bezier(
                parameter,
                control[0].x,
                control[1].x,
                control[2].x,
                control[3].x,
            ) - amount)
                .abs();
            if error < best_error {
                best_error = error;
                best = parameter;
            }
        }
        let scanned = cubic_bezier(best, control[0].y, control[1].y, control[2].y, control[3].y);
        assert_close(easing.evaluate(amount), scanned, 5e-3);
    }
}

#[test]
fn the_spring_settles_without_diverging() {
    let (stiffness, damping) = (170.0_f32, 26.0_f32);
    let mut state = SpringState {
        value: 0.0,
        velocity: 0.0,
    };
    let initial_energy = spring_energy(state, 1.0, stiffness);
    let mut highest = initial_energy;
    for _ in 0..180 {
        state = spring_step(state, 1.0, stiffness, damping, 1.0 / 60.0);
        assert!(state.value.is_finite() && state.velocity.is_finite());
        let energy = spring_energy(state, 1.0, stiffness);
        assert!(
            energy <= highest * 1.02,
            "the spring energy must not grow: {energy} > {highest}"
        );
        highest = highest.max(energy);
    }
    assert!(
        spring_settled(state, 1.0, 0.001),
        "the spring must reach its target, measured {}",
        state.value
    );
    assert!(spring_energy(state, 1.0, stiffness) < initial_energy * 0.01);
}

#[test]
fn a_long_frame_cannot_make_the_spring_diverge() {
    let mut state = SpringState {
        value: 0.0,
        velocity: 0.0,
    };
    state = spring_step(state, 1.0, 170.0, 26.0, 5.0);
    assert!(state.value.is_finite() && state.velocity.is_finite());
    assert!(state.value.abs() < 2.0, "the spring must stay bounded");
    // A non-positive or non-finite step leaves the state alone.
    let idle = spring_step(state, 1.0, 170.0, 26.0, 0.0);
    assert_eq!(idle, state);
    let broken = spring_step(state, 1.0, 170.0, 26.0, f32::NAN);
    assert_eq!(broken, state);
}

#[test]
fn the_spring_energy_fades_without_ever_growing() {
    let (stiffness, damping) = (170.0_f32, 26.0_f32);
    let mut state = SpringState::default();
    let initial = spring_energy(state, 1.0, stiffness);
    let mut previous = initial;
    for _ in 0..3600 {
        state = spring_step(state, 1.0, stiffness, damping, 1.0 / 60.0);
        assert!(state.value.is_finite() && state.velocity.is_finite());
        let energy = spring_energy(state, 1.0, stiffness);
        assert!(
            energy <= previous + initial * 1e-4,
            "the energy must only fade: {energy} > {previous}"
        );
        previous = energy;
    }
    assert!(
        previous < initial * 1e-3,
        "the spring must settle, measured {previous}"
    );
}

#[test]
fn a_long_frame_is_shortened_instead_of_coarsened() {
    let mut stepwise = SpringState::default();
    for _ in 0..MAX_SPRING_STEPS {
        stepwise = spring_step(stepwise, 1.0, 170.0, 26.0, MAX_SPRING_STEP_SECONDS);
    }
    let jumped = spring_step(SpringState::default(), 1.0, 170.0, 26.0, 5.0);
    assert_close(jumped.value, stepwise.value, 1e-4);
    assert_close(jumped.velocity, stepwise.velocity, 1e-3);
}

#[test]
fn the_spring_regime_is_typed() {
    assert_eq!(damping_regime(170.0, 26.0), DampingRegime::Underdamped);
    assert_eq!(damping_regime(100.0, 20.0), DampingRegime::CriticallyDamped);
    assert_eq!(damping_regime(170.0, 40.0), DampingRegime::Overdamped);
    // A spring without a restoring force cannot swing at all.
    assert_eq!(damping_regime(0.0, 10.0), DampingRegime::Overdamped);
}

#[test]
fn the_rounded_rect_distance_is_exact() {
    let half_size = Point::new(10.0, 6.0);
    let radius = 4.0;
    // The centre is as far from the outline as the short half axis.
    assert_close(
        rounded_rect_sdf(Point::new(0.0, 0.0), half_size, radius),
        -6.0,
        1e-4,
    );
    // A point on the straight edge is exactly on the outline.
    assert_close(
        rounded_rect_sdf(Point::new(10.0, 0.0), half_size, radius),
        0.0,
        1e-4,
    );
    assert_close(
        rounded_rect_sdf(Point::new(0.0, 6.0), half_size, radius),
        0.0,
        1e-4,
    );
    // Ten pixels to the right of the edge is ten pixels away.
    assert_close(
        rounded_rect_sdf(Point::new(20.0, 0.0), half_size, radius),
        10.0,
        1e-4,
    );
    // The corner is rounded: the diagonal point is farther than the
    // square corner would be.
    let corner = rounded_rect_sdf(Point::new(10.0, 6.0), half_size, radius);
    assert!(corner > 0.0);
    assert_close(corner, 4.0 * (2.0_f32.sqrt() - 1.0), 1e-4);
}

#[test]
fn the_pill_distance_is_exact() {
    let half_length = 8.0;
    let radius = 3.0;
    assert_close(
        pill_sdf(Point::new(0.0, 0.0), half_length, radius),
        -3.0,
        1e-4,
    );
    assert_close(
        pill_sdf(Point::new(11.0, 0.0), half_length, radius),
        0.0,
        1e-4,
    );
    assert_close(
        pill_sdf(Point::new(0.0, 3.0), half_length, radius),
        0.0,
        1e-4,
    );
    assert_close(
        pill_sdf(Point::new(0.0, 13.0), half_length, radius),
        10.0,
        1e-4,
    );
}

#[test]
fn the_capsule_generalises_the_pill() {
    let half_length = 8.0;
    let radius = 3.0;
    let from = Point::new(-half_length, 0.0);
    let to = Point::new(half_length, 0.0);
    // The capsule of a horizontal segment is the pill of the same segment.
    assert_close(
        capsule_sdf(Point::new(0.0, 0.0), from, to, radius),
        -3.0,
        1e-4,
    );
    assert_close(
        capsule_sdf(Point::new(11.0, 0.0), from, to, radius),
        0.0,
        1e-4,
    );
    assert_close(
        capsule_sdf(Point::new(0.0, 13.0), from, to, radius),
        10.0,
        1e-4,
    );
    assert_close(
        capsule_sdf(Point::new(-9.0, 2.0), from, to, radius),
        -0.764,
        1e-3,
    );
    for point in [
        Point::new(0.0, 0.0),
        Point::new(11.0, 0.0),
        Point::new(0.0, 13.0),
        Point::new(-9.0, 2.0),
        Point::new(3.0, -4.0),
    ] {
        assert_close(
            capsule_sdf(point, from, to, radius),
            pill_sdf(point, half_length, radius),
            1e-4,
        );
    }
    // A vertical capsule is the same shape standing up.
    assert_close(
        capsule_sdf(
            Point::new(0.0, 5.0),
            Point::new(0.0, -2.0),
            Point::new(0.0, 2.0),
            1.0,
        ),
        2.0,
        1e-4,
    );
}

#[test]
fn the_distance_field_has_a_unit_gradient() {
    let half_size = Point::new(10.0, 6.0);
    let radius = 4.0;
    let step = 1e-2;
    for point in [
        Point::new(12.0, 1.5),
        Point::new(1.5, 8.0),
        Point::new(9.0, 5.0),
        Point::new(-13.0, 2.0),
    ] {
        let across = (rounded_rect_sdf(Point::new(point.x + step, point.y), half_size, radius)
            - rounded_rect_sdf(Point::new(point.x - step, point.y), half_size, radius))
            / (2.0 * step);
        let down = (rounded_rect_sdf(Point::new(point.x, point.y + step), half_size, radius)
            - rounded_rect_sdf(Point::new(point.x, point.y - step), half_size, radius))
            / (2.0 * step);
        assert_close(Point::new(across, down).length(), 1.0, 1e-3);
    }
    for point in [Point::new(10.0, 1.0), Point::new(2.0, 5.0)] {
        let across = (pill_sdf(Point::new(point.x + step, point.y), 8.0, 3.0)
            - pill_sdf(Point::new(point.x - step, point.y), 8.0, 3.0))
            / (2.0 * step);
        let down = (pill_sdf(Point::new(point.x, point.y + step), 8.0, 3.0)
            - pill_sdf(Point::new(point.x, point.y - step), 8.0, 3.0))
            / (2.0 * step);
        assert_close(Point::new(across, down).length(), 1.0, 1e-3);
    }
}

#[test]
fn the_rounded_rect_normal_points_outwards() {
    let half_size = Point::new(10.0, 6.0);
    let radius = 4.0;
    let right = rounded_rect_normal(Point::new(12.0, 0.0), half_size, radius);
    assert_close(right.x, 1.0, 1e-5);
    assert_close(right.y, 0.0, 1e-5);
    let top = rounded_rect_normal(Point::new(0.0, 8.0), half_size, radius);
    assert_close(top.x, 0.0, 1e-5);
    assert_close(top.y, 1.0, 1e-5);
    let corner = rounded_rect_normal(Point::new(10.0, 6.0), half_size, radius);
    assert_close(corner.length(), 1.0, 1e-5);
    assert!(corner.x > 0.0 && corner.y > 0.0);
    // The pill normal points away from its segment as well.
    let right = pill_normal(Point::new(11.0, 0.0), 8.0);
    assert_close(right.x, 1.0, 1e-5);
    assert_close(right.y, 0.0, 1e-5);
    let up = pill_normal(Point::new(0.0, 5.0), 8.0);
    assert_close(up.x, 0.0, 1e-5);
    assert_close(up.y, 1.0, 1e-5);
}

#[test]
fn the_ring_marks_the_band_around_the_outline() {
    let half_size = Point::new(10.0, 6.0);
    let radius = 4.0;
    let thickness = 2.0;
    assert_close(
        rounded_rect_ring_sdf(Point::new(10.0, 0.0), half_size, radius, thickness),
        -1.0,
        1e-4,
    );
    assert_close(
        rounded_rect_ring_sdf(Point::new(11.0, 0.0), half_size, radius, thickness),
        0.0,
        1e-4,
    );
    assert_close(
        rounded_rect_ring_sdf(Point::new(9.0, 0.0), half_size, radius, thickness),
        0.0,
        1e-4,
    );
    assert!(rounded_rect_ring_sdf(Point::new(12.0, 0.0), half_size, radius, thickness) > 0.0);
    // The middle of the shape is not part of the band.
    assert!(rounded_rect_ring_sdf(Point::new(0.0, 0.0), half_size, radius, thickness) > 0.0);
}

#[test]
fn the_coverage_falls_smoothly_across_the_outline() {
    assert_close(sdf_coverage(-2.0, 1.0), 1.0, 1e-6);
    assert_close(sdf_coverage(0.0, 1.0), 0.5, 1e-6);
    assert_close(sdf_coverage(2.0, 1.0), 0.0, 1e-6);
    assert_close(sdf_coverage(-1.0, 0.0), 1.0, 1e-6);
    assert_close(sdf_coverage(1.0, 0.0), 0.0, 1e-6);
    let mut previous = 1.0;
    for step in 0..=20 {
        let distance = -1.0 + step as f32 / 10.0;
        let coverage = sdf_coverage(distance, 1.0);
        assert!(
            coverage <= previous + 1e-6,
            "the coverage must not grow: {coverage} > {previous}"
        );
        previous = coverage;
    }
}
