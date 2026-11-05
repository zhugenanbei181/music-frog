//! Contract design tokens rendered as Slint colours.
//!
//! The contract owns the latency ladder *and* the palette the ladder names, so
//! a surface asks for one grade and paints the token that grade points at.
//! Thresholds never live inside a widget again: `PingBadge` used to compare
//! `delay-ms` against its own 100/250 bounds while the shared ladder says
//! 80/160/300, which drew the same probe two different colours on two surfaces.

use infiltrator_contract::design_tokens::{LATENCY_GRADE_PALETTE, LatencyGrade, RgbaToken};

/// Encode one contract token as a Slint colour.
///
/// The contract token is linear 0..1; Slint's encoded constructor takes 0..255
/// u8 channels.
pub(super) fn ink(token: RgbaToken) -> slint::Color {
    let channel = |value: f32| (value.clamp(0.0, 1.0) * 255.0).round() as u8;
    slint::Color::from_argb_u8(
        channel(token.a),
        channel(token.r),
        channel(token.g),
        channel(token.b),
    )
}

/// The colour the shared ladder names for one measured probe.
pub(super) fn latency_ink(delay_ms: Option<u32>) -> slint::Color {
    ink(LatencyGrade::from_millis(delay_ms).token(&LATENCY_GRADE_PALETTE))
}

/// A row that carries no probe at all paints the ladder's unobserved entry,
/// which is the same grey the dashboard already uses for an honest unknown.
pub(super) fn unobserved_ink() -> slint::Color {
    latency_ink(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::design_tokens::LATENCY_GRADE_PALETTE;

    fn assert_ink_is_grade(actual: slint::Color, grade: LatencyGrade) {
        let expected = ink(grade.token(&LATENCY_GRADE_PALETTE));
        assert_eq!(actual, expected, "grade {grade:?} must paint its own token");
    }

    #[test]
    fn every_ladder_step_reaches_the_widget() {
        assert_ink_is_grade(latency_ink(None), LatencyGrade::Unobserved);
        assert_ink_is_grade(latency_ink(Some(80)), LatencyGrade::Excellent);
        assert_ink_is_grade(latency_ink(Some(158)), LatencyGrade::Good);
        assert_ink_is_grade(latency_ink(Some(280)), LatencyGrade::Medium);
        assert_ink_is_grade(latency_ink(Some(4_000)), LatencyGrade::Poor);
    }

    #[test]
    fn the_old_widget_thresholds_no_longer_decide_the_colour() {
        // 158 ms was green under `PingBadge`'s private `< 250` bound and is
        // now the ladder's `Good`; 280 ms was green-ish and is now `Medium`.
        assert_ne!(latency_ink(Some(158)), latency_ink(Some(40)));
        assert_ne!(latency_ink(Some(280)), latency_ink(Some(158)));
        assert_ink_is_grade(latency_ink(Some(161)), LatencyGrade::Medium);
    }

    #[test]
    fn unobserved_matches_the_ladder_timeout_entry() {
        assert_eq!(unobserved_ink(), latency_ink(None));
        assert_ink_is_grade(unobserved_ink(), LatencyGrade::Unobserved);
    }
}
