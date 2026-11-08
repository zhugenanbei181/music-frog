//! The shared motion contract (EFFECTS-003 #10~#13).
//!
//! Motion is a product decision, not a per-surface taste: the same drawer must
//! take the same time on every surface, and the same button must feel the
//! same everywhere. This module carries the four facts that decide it:
//!
//! * the three global durations (#10);
//! * the damped spring the surfaces integrate (#11);
//! * what happens when an animation is interrupted (#12);
//! * how much motion the product shows when the user asks for less (#13).
//!
//! A surface reads the specification and animates; it never invents a
//! duration, a spring constant or an interruption rule of its own.

use std::f32::consts::PI;

use zhuge_ui_core::easing::CubicBezier;

/// The three global motion durations of the product (EFFECTS-003 #10).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MotionDuration {
    /// Immediate feedback: hover, press, a ripple (120ms).
    Fast,
    /// The default transition: a drawer, a panel, a fade (200ms).
    Standard,
    /// Deliberate, attention-carrying motion: a page transition (250ms).
    Deliberate,
}

impl MotionDuration {
    /// Every duration of the ladder.
    pub const ALL: [Self; 3] = [Self::Fast, Self::Standard, Self::Deliberate];

    /// The length of this duration in milliseconds.
    pub const fn millis(self) -> u32 {
        match self {
            Self::Fast => 120,
            Self::Standard => 200,
            Self::Deliberate => 250,
        }
    }
}

/// The damped spring the surfaces integrate their animations with
/// (EFFECTS-003 #11).
///
/// The product's spring is stiff and only just underdamped: it settles fast
/// and carries a single, small overshoot instead of a wobble.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpringParams {
    /// The spring constant, mass-normalised.
    pub stiffness: f32,
    /// The damping constant.
    pub damping: f32,
}

impl SpringParams {
    /// The product's spring.
    pub const DEFAULT: Self = Self {
        stiffness: 170.0,
        damping: 26.0,
    };

    /// The damping that would settle the spring without any overshoot.
    pub fn critical_damping(self) -> f32 {
        2.0 * self.stiffness.sqrt()
    }
}

/// How an interrupted animation hands over to the next one
/// (EFFECTS-003 #12).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MotionInterruptibility {
    /// The next animation continues from the interrupted animation's current
    /// value and velocity: it keeps the interrupted animation's duration and
    /// spring, so a motion that is interrupted mid-flight is never restarted
    /// from the beginning.
    Inherit,
    /// The next animation starts from the beginning with its own duration and
    /// spring.
    Restart,
    /// The interrupted animation jumps to its end value and the next one
    /// starts fresh.
    Snap,
}

impl MotionInterruptibility {
    /// Every interruption rule.
    pub const ALL: [Self; 3] = [Self::Inherit, Self::Restart, Self::Snap];
}

/// How much motion the product shows (EFFECTS-003 #13).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum MotionPreference {
    /// The full motion of the product.
    #[default]
    Full,
    /// Reduced motion: the durations are halved, so no transition reads as a
    /// long travel.
    Reduced,
    /// No motion: every duration collapses to zero and a change is painted
    /// at once.
    None,
}

impl MotionPreference {
    /// Every motion preference.
    pub const ALL: [Self; 3] = [Self::Full, Self::Reduced, Self::None];

    /// The factor every duration is scaled by.
    pub const fn duration_scale(self) -> f32 {
        match self {
            Self::Full => 1.0,
            Self::Reduced => 0.5,
            Self::None => 0.0,
        }
    }
}

/// The complete motion description of one animated transition.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MotionSpec {
    /// How long the transition runs at full motion.
    pub duration: MotionDuration,
    /// The spring the transition is integrated with.
    pub spring: SpringParams,
    /// What happens when the transition is interrupted.
    pub interruptibility: MotionInterruptibility,
}

impl MotionSpec {
    /// The product's default transition.
    pub const DEFAULT: Self = Self {
        duration: MotionDuration::Standard,
        spring: SpringParams::DEFAULT,
        interruptibility: MotionInterruptibility::Inherit,
    };

    /// The length of the transition under one motion preference.
    pub fn duration_ms(&self, preference: MotionPreference) -> u32 {
        let scaled = self.duration.millis() as f32 * preference.duration_scale();
        scaled.round() as u32
    }

    /// How this transition hands over to the next one.
    pub const fn handover(&self, next: MotionSpec) -> MotionHandover {
        match self.interruptibility {
            MotionInterruptibility::Inherit => MotionHandover::Continue(MotionSpec {
                duration: self.duration,
                spring: self.spring,
                interruptibility: next.interruptibility,
            }),
            MotionInterruptibility::Restart => MotionHandover::Restart(next),
            MotionInterruptibility::Snap => MotionHandover::Snap(next),
        }
    }
}

impl Default for MotionSpec {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// The result of interrupting one transition with the next.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MotionHandover {
    /// The next transition continues the interrupted motion.
    Continue(MotionSpec),
    /// The next transition starts from the beginning with its own motion.
    Restart(MotionSpec),
    /// The interrupted transition snaps to its end value and the next one
    /// starts fresh.
    Snap(MotionSpec),
}

impl MotionHandover {
    /// The specification the next transition runs with.
    pub const fn next(&self) -> MotionSpec {
        match self {
            Self::Continue(spec) | Self::Restart(spec) | Self::Snap(spec) => *spec,
        }
    }
}

/// The four Material Design 3 easing curves this product binds to the shared owner.
///
/// The curve values remain product facts. Their parameter resolution is the
/// single `zhuge_ui_core::easing::CubicBezier` implementation, so contract owns
/// the identity of the curve and the shared crate owns the numeric behavior.
const STANDARD: CubicBezier = CubicBezier::new(0.2, 0.0, 0.0, 1.0);
const EMPHASIZED: CubicBezier = CubicBezier::new(0.05, 0.7, 0.1, 1.0);
const DECELERATED: CubicBezier = CubicBezier::new(0.0, 0.0, 0.2, 1.0);
const ACCELERATED: CubicBezier = CubicBezier::new(0.3, 0.0, 1.0, 1.0);

/// Material Design 3 and physical easing curves.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum EasingCurve {
    /// The standard responsive easing for elements moving on screen.
    #[default]
    Standard,
    /// Emphasized easing for attention-demanding transitions (e.g. modals).
    Emphasized,
    /// Decelerated easing for incoming elements entering the screen.
    Decelerated,
    /// Accelerated easing for outgoing elements leaving the screen.
    Accelerated,
    /// Linear progression without acceleration or deceleration.
    Linear,
    /// Damped spring physics curve.
    Spring,
}

impl EasingCurve {
    /// Evaluate the progress value for normalized time `t` in `0.0..=1.0`.
    pub fn evaluate(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Self::Standard => STANDARD.sample(t),
            Self::Emphasized => EMPHASIZED.sample(t),
            Self::Decelerated => DECELERATED.sample(t),
            Self::Accelerated => ACCELERATED.sample(t),
            Self::Linear => t,
            Self::Spring => {
                let p = t * PI;
                let decay = (-3.5 * t).exp();
                1.0 - decay * (p * 2.5).cos()
            }
        }
    }
}

/// Material Design 3 transition patterns (FLPARITY-017).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TransitionPattern {
    /// Page route transition with subtle scale and cross-fade.
    #[default]
    FadeThrough,
    /// Horizontal shared axis transition for forward/backward navigation.
    SharedAxisX,
    /// Vertical shared axis transition for hierarchical drill-down/up.
    SharedAxisY,
    /// Drawer slide from edge with background scrim elevation.
    DrawerSlide,
    /// Modal dialog pop-up with vertical lift, scale-up, and backdrop.
    ModalElevation,
    /// Expanding a card/tile into a full-page view container.
    ContainerTransform,
}

/// Transition direction indicating enter or exit phase.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TransitionDirection {
    /// The element is entering the screen.
    #[default]
    Enter,
    /// The element is leaving the screen.
    Exit,
}

/// The interpolated visual parameters of a transition at a given point in time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TransitionProgress {
    /// Normalized transition progress `0.0..=1.0`.
    pub progress: f32,
    /// Element opacity `0.0..=1.0`.
    pub opacity: f32,
    /// Scaling factor (e.g. `0.92..=1.0`).
    pub scale: f32,
    /// Horizontal translation offset in logical pixels.
    pub offset_x_px: f32,
    /// Vertical translation offset in logical pixels.
    pub offset_y_px: f32,
    /// Modal backdrop scrim opacity `0.0..=1.0`.
    pub scrim_opacity: f32,
    /// Whether the transition has reached its terminal state.
    pub is_completed: bool,
}

/// The complete typed transition specification.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TransitionSpec {
    pub pattern: TransitionPattern,
    pub direction: TransitionDirection,
    pub preference: MotionPreference,
}

impl TransitionSpec {
    /// Construct a new transition specification.
    pub const fn new(
        pattern: TransitionPattern,
        direction: TransitionDirection,
        preference: MotionPreference,
    ) -> Self {
        Self {
            pattern,
            direction,
            preference,
        }
    }

    /// Compute the effective duration in milliseconds.
    pub fn duration_ms(&self) -> u32 {
        let base = match self.pattern {
            TransitionPattern::FadeThrough
            | TransitionPattern::SharedAxisX
            | TransitionPattern::SharedAxisY => MotionDuration::Deliberate.millis(),
            TransitionPattern::DrawerSlide | TransitionPattern::ModalElevation => {
                MotionDuration::Standard.millis()
            }
            TransitionPattern::ContainerTransform => 300,
        };
        (base as f32 * self.preference.duration_scale()).round() as u32
    }

    /// The easing curve associated with this pattern and direction.
    pub const fn easing(&self) -> EasingCurve {
        match self.pattern {
            TransitionPattern::FadeThrough
            | TransitionPattern::SharedAxisX
            | TransitionPattern::SharedAxisY
            | TransitionPattern::ContainerTransform => EasingCurve::Standard,
            TransitionPattern::DrawerSlide => match self.direction {
                TransitionDirection::Enter => EasingCurve::Decelerated,
                TransitionDirection::Exit => EasingCurve::Accelerated,
            },
            TransitionPattern::ModalElevation => EasingCurve::Emphasized,
        }
    }

    /// Interpolate visual properties for an elapsed time in milliseconds.
    pub fn interpolate(&self, elapsed_ms: u32) -> TransitionProgress {
        let total_ms = self.duration_ms();
        if total_ms == 0 || elapsed_ms >= total_ms {
            let is_enter = matches!(self.direction, TransitionDirection::Enter);
            return TransitionProgress {
                progress: 1.0,
                opacity: if is_enter { 1.0 } else { 0.0 },
                scale: 1.0,
                offset_x_px: 0.0,
                offset_y_px: 0.0,
                scrim_opacity: if is_enter
                    && matches!(
                        self.pattern,
                        TransitionPattern::DrawerSlide | TransitionPattern::ModalElevation
                    ) {
                    0.5
                } else {
                    0.0
                },
                is_completed: true,
            };
        }

        let t = (elapsed_ms as f32 / total_ms as f32).clamp(0.0, 1.0);
        let eased = self.easing().evaluate(t);
        let is_enter = matches!(self.direction, TransitionDirection::Enter);
        let is_reduced = matches!(self.preference, MotionPreference::Reduced);
        let offset_factor = if is_reduced { 0.2 } else { 1.0 };

        match self.pattern {
            TransitionPattern::FadeThrough => {
                let opacity = if is_enter { eased } else { 1.0 - eased };
                let scale = if is_reduced {
                    1.0
                } else if is_enter {
                    0.94 + 0.06 * eased
                } else {
                    1.0 - 0.04 * eased
                };
                TransitionProgress {
                    progress: eased,
                    opacity,
                    scale,
                    offset_x_px: 0.0,
                    offset_y_px: 0.0,
                    scrim_opacity: 0.0,
                    is_completed: false,
                }
            }
            TransitionPattern::SharedAxisX => {
                let opacity = if is_enter { eased } else { 1.0 - eased };
                let offset_x = if is_enter {
                    32.0 * (1.0 - eased) * offset_factor
                } else {
                    -32.0 * eased * offset_factor
                };
                TransitionProgress {
                    progress: eased,
                    opacity,
                    scale: 1.0,
                    offset_x_px: offset_x,
                    offset_y_px: 0.0,
                    scrim_opacity: 0.0,
                    is_completed: false,
                }
            }
            TransitionPattern::SharedAxisY => {
                let opacity = if is_enter { eased } else { 1.0 - eased };
                let offset_y = if is_enter {
                    24.0 * (1.0 - eased) * offset_factor
                } else {
                    -24.0 * eased * offset_factor
                };
                TransitionProgress {
                    progress: eased,
                    opacity,
                    scale: 1.0,
                    offset_x_px: 0.0,
                    offset_y_px: offset_y,
                    scrim_opacity: 0.0,
                    is_completed: false,
                }
            }
            TransitionPattern::DrawerSlide => {
                let max_offset = 280.0 * offset_factor;
                let offset_x = if is_enter {
                    -max_offset * (1.0 - eased)
                } else {
                    -max_offset * eased
                };
                let scrim = if is_enter {
                    0.5 * eased
                } else {
                    0.5 * (1.0 - eased)
                };
                TransitionProgress {
                    progress: eased,
                    opacity: 1.0,
                    scale: 1.0,
                    offset_x_px: offset_x,
                    offset_y_px: 0.0,
                    scrim_opacity: scrim,
                    is_completed: false,
                }
            }
            TransitionPattern::ModalElevation => {
                let opacity = if is_enter { eased } else { 1.0 - eased };
                let scale = if is_reduced {
                    1.0
                } else if is_enter {
                    0.90 + 0.10 * eased
                } else {
                    1.0 - 0.06 * eased
                };
                let offset_y = if is_enter {
                    16.0 * (1.0 - eased) * offset_factor
                } else {
                    8.0 * eased * offset_factor
                };
                let scrim = if is_enter {
                    0.6 * eased
                } else {
                    0.6 * (1.0 - eased)
                };
                TransitionProgress {
                    progress: eased,
                    opacity,
                    scale,
                    offset_x_px: 0.0,
                    offset_y_px: offset_y,
                    scrim_opacity: scrim,
                    is_completed: false,
                }
            }
            TransitionPattern::ContainerTransform => {
                let opacity = if is_enter { eased } else { 1.0 - eased };
                let scale = if is_reduced {
                    1.0
                } else if is_enter {
                    0.85 + 0.15 * eased
                } else {
                    1.0 - 0.15 * eased
                };
                TransitionProgress {
                    progress: eased,
                    opacity,
                    scale,
                    offset_x_px: 0.0,
                    offset_y_px: 0.0,
                    scrim_opacity: 0.0,
                    is_completed: false,
                }
            }
        }
    }
}

/// Design tokens collection for motion and micro-interactions.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MotionTokens;

impl MotionTokens {
    /// Fast duration (120ms): button clicks, toggle switches, hover feedback.
    pub const FAST_MS: u32 = MotionDuration::Fast.millis();
    /// Standard duration (200ms): drawer slide, modal reveals, accordion fold.
    pub const STANDARD_MS: u32 = MotionDuration::Standard.millis();
    /// Deliberate duration (250ms): page route transitions, shared axis travel.
    pub const DELIBERATE_MS: u32 = MotionDuration::Deliberate.millis();
    /// Expanded duration (350ms): full-screen card transformations.
    pub const EXPANDED_MS: u32 = 350;

    /// Default snappy spring for button pop and knob bounce.
    pub const SPRING_SNAPPY: SpringParams = SpringParams {
        stiffness: 320.0,
        damping: 22.0,
    };

    /// Default gentle spring for drawer and backdrop elevation.
    pub const SPRING_GENTLE: SpringParams = SpringParams {
        stiffness: 120.0,
        damping: 18.0,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_duration_ladder_is_the_product_ladder() {
        assert_eq!(MotionDuration::Fast.millis(), 120);
        assert_eq!(MotionDuration::Standard.millis(), 200);
        assert_eq!(MotionDuration::Deliberate.millis(), 250);
        let millis = MotionDuration::ALL.map(|duration| duration.millis());
        for pair in millis.windows(2) {
            assert!(pair[0] < pair[1], "the durations must be ordered");
        }
    }

    #[test]
    fn the_product_spring_is_stiff_and_just_underdamped() {
        let spring = SpringParams::DEFAULT;
        assert_eq!(spring.stiffness, 170.0);
        assert_eq!(spring.damping, 26.0);
        let critical = spring.critical_damping();
        assert!(
            spring.damping < critical,
            "the spring must carry one small overshoot"
        );
        assert!(
            spring.damping > critical * 0.9,
            "the spring must not be damped into a crawl"
        );
    }

    #[test]
    fn the_reduced_motion_preference_shortens_and_cancels() {
        let spec = MotionSpec::DEFAULT;
        assert_eq!(spec.duration_ms(MotionPreference::Full), 200);
        assert_eq!(spec.duration_ms(MotionPreference::Reduced), 100);
        assert_eq!(spec.duration_ms(MotionPreference::None), 0);
        for preference in MotionPreference::ALL {
            assert!(spec.duration_ms(preference) <= MotionDuration::Standard.millis());
        }
    }

    #[test]
    fn an_interrupted_motion_hands_over_by_its_own_rule() {
        let inherited = MotionSpec {
            interruptibility: MotionInterruptibility::Inherit,
            ..MotionSpec::DEFAULT
        };
        let restarted = MotionSpec {
            interruptibility: MotionInterruptibility::Restart,
            ..MotionSpec::DEFAULT
        };
        let snapped = MotionSpec {
            interruptibility: MotionInterruptibility::Snap,
            ..MotionSpec::DEFAULT
        };
        let next = MotionSpec {
            duration: MotionDuration::Fast,
            ..MotionSpec::DEFAULT
        };

        let handover = inherited.handover(next);
        assert_eq!(
            handover.next().duration,
            MotionDuration::Standard,
            "an inherited motion keeps the interrupted duration"
        );
        assert!(matches!(handover, MotionHandover::Continue(_)));

        let handover = restarted.handover(next);
        assert_eq!(handover.next().duration, MotionDuration::Fast);
        assert!(matches!(handover, MotionHandover::Restart(_)));

        let handover = snapped.handover(next);
        assert_eq!(handover.next().duration, MotionDuration::Fast);
        assert!(matches!(handover, MotionHandover::Snap(_)));
    }

    #[test]
    fn the_default_spec_is_the_product_default() {
        assert_eq!(MotionSpec::default(), MotionSpec::DEFAULT);
        assert_eq!(MotionSpec::DEFAULT.duration, MotionDuration::Standard);
        assert_eq!(
            MotionSpec::DEFAULT.spring,
            SpringParams::DEFAULT,
            "the default motion uses the product spring"
        );
        assert_eq!(
            MotionSpec::DEFAULT.interruptibility,
            MotionInterruptibility::Inherit
        );
    }

    #[test]
    fn the_shared_owner_keeps_the_m3_curves_monotonic_and_bounded() {
        for curve in [STANDARD, EMPHASIZED, DECELERATED, ACCELERATED] {
            assert_eq!(curve.sample(0.0), 0.0);
            assert_eq!(curve.sample(1.0), 1.0);

            let mut prev = 0.0;
            for step in 1..=20 {
                let t = step as f32 / 20.0;
                let val = curve.sample(t);
                assert!(
                    val >= prev - 1e-4,
                    "easing curve should be monotonic non-decreasing: t={t}, val={val}, prev={prev}"
                );
                assert!((0.0..=1.0).contains(&val));
                prev = val;
            }
        }
    }

    #[test]
    fn the_shared_owner_hits_the_exact_m3_standard_midpoint() {
        assert_eq!(
            STANDARD.sample(0.2),
            0.5,
            "the old 1e-5 early stop returned 0.49999994; the shared owner is exact here"
        );
        assert_eq!(EasingCurve::Standard.evaluate(0.2), 0.5);
    }

    #[test]
    fn the_transition_grid_keeps_its_display_contract() {
        for pattern in [
            TransitionPattern::FadeThrough,
            TransitionPattern::SharedAxisX,
            TransitionPattern::SharedAxisY,
            TransitionPattern::DrawerSlide,
            TransitionPattern::ModalElevation,
            TransitionPattern::ContainerTransform,
        ] {
            for direction in [TransitionDirection::Enter, TransitionDirection::Exit] {
                for preference in [
                    MotionPreference::Full,
                    MotionPreference::Reduced,
                    MotionPreference::None,
                ] {
                    let spec = TransitionSpec::new(pattern, direction, preference);
                    let terminal = spec.interpolate(spec.duration_ms());
                    assert!(terminal.is_completed);
                    assert_eq!(terminal.progress, 1.0);
                    assert_eq!(
                        terminal.opacity,
                        if matches!(direction, TransitionDirection::Enter) {
                            1.0
                        } else {
                            0.0
                        }
                    );
                    assert_eq!(terminal.scale, 1.0);
                    assert_eq!(terminal.offset_x_px, 0.0);
                    assert_eq!(terminal.offset_y_px, 0.0);

                    let mut previous_progress = -1.0;
                    for elapsed_ms in 0..spec.duration_ms() {
                        let frame = spec.interpolate(elapsed_ms);
                        assert!(!frame.is_completed);
                        assert!((0.0..=1.0).contains(&frame.progress));
                        assert!(frame.progress >= previous_progress);
                        previous_progress = frame.progress;
                        assert!(frame.offset_x_px.abs() <= 280.0);
                        assert!(frame.offset_y_px.abs() <= 24.0);
                        assert!((0.0..=1.0).contains(&frame.opacity));
                        assert!((0.0..=1.0).contains(&frame.scrim_opacity));
                    }
                }
            }
        }
    }

    #[test]
    fn test_easing_curves_evaluation() {
        for easing in [
            EasingCurve::Standard,
            EasingCurve::Emphasized,
            EasingCurve::Decelerated,
            EasingCurve::Accelerated,
            EasingCurve::Linear,
        ] {
            assert_eq!(easing.evaluate(0.0), 0.0);
            assert_eq!(easing.evaluate(1.0), 1.0);
            let mid = easing.evaluate(0.5);
            assert!(mid > 0.0 && mid < 1.0);
        }

        let spring = EasingCurve::Spring;
        let start = spring.evaluate(0.0);
        assert!(start.abs() < 1e-3);
    }

    #[test]
    fn test_transition_spec_durations_and_curves() {
        let spec_fade = TransitionSpec::new(
            TransitionPattern::FadeThrough,
            TransitionDirection::Enter,
            MotionPreference::Full,
        );
        assert_eq!(spec_fade.duration_ms(), 250);
        assert_eq!(spec_fade.easing(), EasingCurve::Standard);

        let spec_drawer = TransitionSpec::new(
            TransitionPattern::DrawerSlide,
            TransitionDirection::Enter,
            MotionPreference::Full,
        );
        assert_eq!(spec_drawer.duration_ms(), 200);
        assert_eq!(spec_drawer.easing(), EasingCurve::Decelerated);

        let spec_drawer_exit = TransitionSpec::new(
            TransitionPattern::DrawerSlide,
            TransitionDirection::Exit,
            MotionPreference::Full,
        );
        assert_eq!(spec_drawer_exit.easing(), EasingCurve::Accelerated);
    }

    #[test]
    fn test_transition_spec_interpolations() {
        let spec = TransitionSpec::new(
            TransitionPattern::FadeThrough,
            TransitionDirection::Enter,
            MotionPreference::Full,
        );

        let start = spec.interpolate(0);
        assert_eq!(start.opacity, 0.0);
        assert!(!start.is_completed);
        assert!(start.scale < 1.0 && start.scale >= 0.94);

        let mid = spec.interpolate(125);
        assert!(mid.opacity > 0.0 && mid.opacity < 1.0);
        assert!(!mid.is_completed);

        let end = spec.interpolate(250);
        assert_eq!(end.opacity, 1.0);
        assert_eq!(end.scale, 1.0);
        assert!(end.is_completed);
    }

    #[test]
    fn test_reduced_motion_and_none_preference() {
        let spec_none = TransitionSpec::new(
            TransitionPattern::ModalElevation,
            TransitionDirection::Enter,
            MotionPreference::None,
        );
        assert_eq!(spec_none.duration_ms(), 0);
        let step = spec_none.interpolate(0);
        assert!(step.is_completed);
        assert_eq!(step.opacity, 1.0);
        assert_eq!(step.scale, 1.0);

        let spec_reduced = TransitionSpec::new(
            TransitionPattern::SharedAxisX,
            TransitionDirection::Enter,
            MotionPreference::Reduced,
        );
        assert_eq!(spec_reduced.duration_ms(), 125);
        let mid = spec_reduced.interpolate(60);
        assert!(!mid.is_completed);
        // reduced motion scales offsets down by factor of 0.2
        assert!(mid.offset_x_px <= 32.0 * 0.2);
    }

    #[test]
    fn test_motion_tokens_constants() {
        assert_eq!(MotionTokens::FAST_MS, 120);
        assert_eq!(MotionTokens::STANDARD_MS, 200);
        assert_eq!(MotionTokens::DELIBERATE_MS, 250);
        assert_eq!(MotionTokens::EXPANDED_MS, 350);
        let snappy = MotionTokens::SPRING_SNAPPY;
        let gentle = MotionTokens::SPRING_GENTLE;
        assert!(snappy.stiffness > gentle.stiffness);
    }
}
