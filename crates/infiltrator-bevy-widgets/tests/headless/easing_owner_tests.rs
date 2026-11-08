//! The easing facts a surface paints with come from their shared owners (UIFRAME parity).
//!
//! Two shapes reach the screen through this crate: the anti-aliasing ramp of an SDF
//! edge or a drop shadow, and the cubic-Bézier curve an animation eases along. Both
//! are owned by `zhuge-ui-core::easing`; these tests pin the composition (the surface
//! inverts, clamps and declares its degenerate answer; the owner supplies the shape),
//! which is what the retired local `smoothstep` spellings and the retired
//! `infiltrator_shared::effect_math::BezierEasing` used to re-derive.

use infiltrator_bevy_widgets::shader_fx::{
    AnalyticalDropShadow, MIN_EDGE_AA_PX, sdf_edge_coverage,
};
use zhuge_ui_core::easing::{CubicBezier, smoothstep};

/// The band centre of the edge ramp, exactly as the surface declares it.
fn edge_ramp_parameter(distance: f32, derivative: f32) -> f32 {
    let width = derivative.abs().max(MIN_EDGE_AA_PX);
    ((distance + width * 0.5) / width).clamp(0.0, 1.0)
}

#[test]
fn the_edge_coverage_is_the_owner_ramp_read_backwards() {
    for derivative in [-3.0_f32, -0.2, 0.0, 0.2, 1.0, 7.5] {
        for step in 0..=24u32 {
            let distance = (step as f32 / 24.0 - 0.5) * 20.0;
            let parameter = edge_ramp_parameter(distance, derivative);
            let expected = (1.0 - smoothstep(parameter)).clamp(0.0, 1.0);
            assert_eq!(
                sdf_edge_coverage(distance, derivative),
                expected,
                "the surface owns the inversion, the owner owns the ramp: d={distance} w={derivative}"
            );
        }
    }
}

#[test]
fn the_edge_coverage_never_reverses_and_stays_inside_its_band() {
    let derivative = 1.4_f32;
    let mut previous = 2.0;
    for step in 0..=64u32 {
        let distance = (step as f32 / 64.0 - 0.5) * 12.0;
        let coverage = sdf_edge_coverage(distance, derivative);
        assert!(
            coverage <= previous + 1e-6,
            "coverage only ever falls as the distance grows: {coverage} after {previous}"
        );
        assert!((0.0..=1.0).contains(&coverage));
        previous = coverage;
    }
}

#[test]
fn an_untraceable_edge_declares_the_outside_answer() {
    assert_eq!(sdf_edge_coverage(f32::NAN, 1.0), 0.0);
    // A derivative that is not a number never reaches the ramp: the anti-aliasing
    // floor answers it, so the edge keeps its documented half coverage.
    assert_eq!(sdf_edge_coverage(0.0, f32::NAN), 0.5);
    assert_eq!(sdf_edge_coverage(3.0, f32::NAN), 0.0);
    assert_eq!(sdf_edge_coverage(1.0, f32::INFINITY), 0.0);
    assert_eq!(sdf_edge_coverage(f32::NEG_INFINITY, 1.0), 1.0);
    assert_eq!(sdf_edge_coverage(f32::INFINITY, 1.0), 0.0);
}

#[test]
fn the_shadow_falloff_is_the_owner_ramp_with_its_own_ends() {
    let shadow = AnalyticalDropShadow::elevation_medium();
    assert_eq!(shadow.falloff_alpha(0.0), 1.0);
    assert_eq!(shadow.falloff_alpha(shadow.blur_radius), 0.0);
    let mut previous = 2.0;
    for step in 0..=32u32 {
        let distance = shadow.blur_radius * step as f32 / 32.0;
        let alpha = shadow.falloff_alpha(distance);
        assert!(
            alpha <= previous + 1e-6,
            "the falloff only ever fades: {alpha} after {previous}"
        );
        assert!((0.0..=1.0).contains(&alpha));
        previous = alpha;
    }
    let mid = shadow.falloff_alpha(shadow.blur_radius * 0.5);
    assert!(
        (mid - (1.0 - smoothstep(0.5))).abs() < 1e-6,
        "the band centre is the owner's ramp at its midpoint"
    );
}

#[test]
fn an_untraceable_falloff_declares_the_outside_answer() {
    let shadow = AnalyticalDropShadow::elevation_high();
    assert_eq!(shadow.falloff_alpha(f32::NAN), 0.0);
    let broken = AnalyticalDropShadow {
        blur_radius: f32::NAN,
        ..shadow
    };
    assert_eq!(broken.falloff_alpha(4.0), 0.0);
}

#[test]
fn the_shared_owner_solves_the_easing_curve_a_surface_animates_along() {
    for curve in [
        CubicBezier::LINEAR,
        CubicBezier::EASE,
        CubicBezier::EASE_IN,
        CubicBezier::EASE_OUT,
        CubicBezier::EASE_IN_OUT,
        CubicBezier::EASE_OUT_CUBIC,
    ] {
        assert_eq!(curve.sample(0.0), 0.0, "an eased transition starts at rest");
        assert_eq!(curve.sample(1.0), 1.0, "an eased transition ends finished");
        let mut previous = -1.0;
        for step in 1..=40u32 {
            let progress = step as f32 / 40.0;
            let eased = curve.sample(progress);
            assert!(
                eased >= previous - 1e-4,
                "the solved curve never runs backwards: t={progress} y={eased} after {previous}"
            );
            assert!(
                (0.0..=1.0).contains(&eased),
                "the solve stays inside the band"
            );
            previous = eased;
        }
    }
    assert!(
        (CubicBezier::LINEAR.sample(0.37) - 0.37).abs() < 1e-5,
        "the linear curve is the identity up to the solver's tolerance"
    );
    assert!(
        CubicBezier::EASE_IN.sample(0.5) < 0.5,
        "an accelerating curve is behind its progress at the midpoint"
    );
    assert!(
        CubicBezier::EASE_OUT.sample(0.5) > 0.5,
        "a decelerating curve is ahead of its progress at the midpoint"
    );
}
