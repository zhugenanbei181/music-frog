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
}
