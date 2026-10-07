//! test-intent: behavior
use super::LogFollowState;
#[test]
fn growth_does_not_stop_following_but_actual_history_scroll_does_and_resume_returns_to_end() {
    let mut follow = LogFollowState::default();
    follow.observe_viewport(400.0, 600.0, 200.0);
    assert!(follow.should_follow());
    follow.observe_viewport(400.0, 700.0, 200.0);
    assert!(follow.should_follow());
    assert_eq!(follow.restored_offset(500.0), 500.0);
    follow.observe_viewport(350.0, 700.0, 200.0);
    assert!(!follow.should_follow());
    assert_eq!(follow.restored_offset(500.0), 350.0);
    assert_eq!(follow.label_key(), "logs_scroll_resume");
    follow.toggle_follow();
    assert!(follow.should_follow());
    assert_eq!(follow.restored_offset(500.0), 500.0);
}
#[test]
fn explicit_lock_survives_end_scroll_resize_and_invalid_geometry_until_user_resumes() {
    let mut follow = LogFollowState::default();
    follow.observe_viewport(400.0, 600.0, 200.0);
    follow.toggle_follow();
    follow.observe_viewport(500.0, 700.0, 200.0);
    assert!(!follow.should_follow());
    follow.observe_viewport(f32::NAN, 900.0, 200.0);
    assert_eq!(follow.restored_offset(700.0), 500.0);
    follow.observe_viewport(0.0, 100.0, 200.0);
    assert!(!follow.should_follow());
    follow.toggle_follow();
    assert!(follow.should_follow());
    assert_eq!(follow.label_key(), "logs_scroll_lock");
}
