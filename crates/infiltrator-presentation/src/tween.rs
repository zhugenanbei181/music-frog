//! The numeric tween that rolls a changing number to its new value
//! (EFFECTS-011 #39).
//!
//! A rate that jumps from `0` to `12 MB/s` in one frame reads as a glitch: the
//! digits change, the row does not move, and the eye sees the number flicker
//! instead of roll. The tween turns the jump into a short, continuous
//! movement — 120 milliseconds — so the number arrives where it belongs.

/// The length of the numeric roll, in milliseconds.
pub const NUMERIC_TWEEN_MS: f32 = 120.0;

/// A number that rolls from one value to the next.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NumericTween {
    from: f64,
    to: f64,
    elapsed_ms: f32,
    duration_ms: f32,
}

impl NumericTween {
    /// A tween that starts at `from` and rolls to `to`.
    pub const fn new(from: f64, to: f64, duration_ms: f32) -> Self {
        Self {
            from,
            to,
            elapsed_ms: 0.0,
            duration_ms,
        }
    }

    /// The value the tween currently shows.
    pub fn value(&self) -> f64 {
        if self.duration_ms <= 0.0 {
            return self.to;
        }
        let progress = (self.elapsed_ms / self.duration_ms).clamp(0.0, 1.0) as f64;
        let eased = 1.0 - (1.0 - progress) * (1.0 - progress);
        self.from + (self.to - self.from) * eased
    }

    /// The value the tween rolls to.
    pub const fn target(&self) -> f64 {
        self.to
    }

    /// Whether the tween has arrived.
    pub fn is_settled(&self) -> bool {
        self.duration_ms <= 0.0 || self.elapsed_ms >= self.duration_ms
    }

    /// Advance the tween and report whether it arrived.
    pub fn advance(&mut self, delta_ms: f32) -> bool {
        if !delta_ms.is_finite() || delta_ms <= 0.0 {
            return self.is_settled();
        }
        self.elapsed_ms += delta_ms;
        if self.elapsed_ms >= self.duration_ms {
            self.elapsed_ms = self.duration_ms;
            return true;
        }
        false
    }

    /// Roll to another value, continuing from the value on screen.
    ///
    /// A value that changes while the tween is still rolling does not restart
    /// the roll: the number continues from where it is, so it never jumps
    /// back to the beginning.
    pub fn retarget(&mut self, to: f64) {
        self.from = self.value();
        self.to = to;
        self.elapsed_ms = 0.0;
    }

    /// Jump to the target without rolling.
    pub fn settle(&mut self) {
        self.from = self.to;
        self.elapsed_ms = self.duration_ms;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_roll_is_continuous_and_arrives() {
        let mut tween = NumericTween::new(0.0, 100.0, NUMERIC_TWEEN_MS);
        assert_eq!(tween.value(), 0.0);
        let mut previous = tween.value();
        for _ in 0..12 {
            tween.advance(10.0);
            let current = tween.value();
            assert!(
                current > previous,
                "the roll must move forward: {previous} -> {current}"
            );
            previous = current;
        }
        assert!(tween.is_settled());
        assert_eq!(tween.value(), 100.0);
    }

    #[test]
    fn a_jump_does_not_restart_the_roll() {
        let mut tween = NumericTween::new(0.0, 100.0, NUMERIC_TWEEN_MS);
        tween.advance(60.0);
        let halfway = tween.value();
        tween.retarget(200.0);
        assert_eq!(
            tween.value(),
            halfway,
            "the new roll continues from the value on screen"
        );
        tween.advance(NUMERIC_TWEEN_MS);
        assert_eq!(tween.value(), 200.0);
    }

    #[test]
    fn a_zero_length_roll_arrives_at_once() {
        let mut tween = NumericTween::new(0.0, 42.0, 0.0);
        assert_eq!(tween.value(), 42.0);
        assert!(tween.is_settled());
        assert!(tween.advance(1.0));
    }

    #[test]
    fn settling_jumps_to_the_target() {
        let mut tween = NumericTween::new(0.0, 10.0, NUMERIC_TWEEN_MS);
        tween.advance(30.0);
        tween.settle();
        assert_eq!(tween.value(), 10.0);
        assert!(tween.is_settled());
    }
}
