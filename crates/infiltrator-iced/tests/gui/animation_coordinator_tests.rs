//! Animation coordinator tests (mounted from `src/animation.rs`): the
//! Material 3 route transition projection starts from zero opacity, passes
//! through mid-fade and settles at full opacity with the frame gate closed.

use super::*;

#[test]
fn route_transition_runs_from_zero_to_full_opacity() {
    let mut coord = AnimationCoordinator::default();
    let now = Instant::now();
    assert!(!coord.is_animating(now));

    coord.route_changed(now);
    assert!(coord.is_animating(now));
    let trans_start = coord.route_transition(now);
    assert!(trans_start.is_animating);
    assert_eq!(trans_start.opacity, 0.0);

    let half = now + Duration::from_millis(125);
    let trans_half = coord.route_transition(half);
    assert!(trans_half.is_animating);
    assert!(trans_half.opacity > 0.0);

    let completed_time = now + Duration::from_millis(300);
    let trans_done = coord.route_transition(completed_time);
    assert!(!trans_done.is_animating);
    assert_eq!(trans_done.opacity, 1.0);
    assert_eq!(trans_done.scale, 1.0);
}
