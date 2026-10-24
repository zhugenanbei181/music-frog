//! The animation coordinator on the official `iced::Animation` driver
//! (EFFECTS-029 #116-#118): fade windows, the on-demand frame gate, and the
//! drawer's exit retention.
//! test-intent: behavior
use infiltrator_iced::animation::{AnimationCoordinator, Presence, DRAWER_FADE, ROUTE_FADE};
use std::time::{Duration, Instant};

#[test]
fn a_route_change_fades_in_over_the_full_window() {
    let mut coordinator = AnimationCoordinator::default();
    let start = Instant::now();
    coordinator.route_changed(start);

    assert!((coordinator.route_opacity(start) - 0.0).abs() < 1e-3);
    let half = coordinator.route_opacity(start + Duration::from_millis(150));
    assert!((half - 0.5).abs() < 1e-3, "linear midpoint, got {half}");
    let end = coordinator.route_opacity(start + ROUTE_FADE);
    assert!((end - 1.0).abs() < 1e-3);
    assert!(!coordinator.is_animating(start + ROUTE_FADE));
}

#[test]
fn a_settled_shell_never_ticks_frames() {
    let coordinator = AnimationCoordinator::default();
    let now = Instant::now();
    assert!(!coordinator.is_animating(now));
    assert!((coordinator.route_opacity(now) - 1.0).abs() < 1e-6);
    assert!(!coordinator.drawer_presented(now));
}

#[test]
fn a_closing_drawer_keeps_its_exit_animation() {
    let mut coordinator = AnimationCoordinator::default();
    let start = Instant::now();
    coordinator.drawer_redirect(true, start);
    assert!(coordinator.drawer_presented(start));

    let close_at = start + Duration::from_secs(5);
    coordinator.drawer_redirect(false, close_at);

    // Still presented mid-exit, at a fading opacity (#118).
    let mid = close_at + Duration::from_millis(100);
    assert!(coordinator.drawer_presented(mid), "#118 exit retention");
    let opacity = coordinator.drawer_opacity(mid);
    assert!((opacity - 0.5).abs() < 1e-3, "linear exit midpoint, got {opacity}");

    // Gone once the exit fade settles.
    let settled = close_at + DRAWER_FADE + Duration::from_millis(1);
    assert!(!coordinator.drawer_presented(settled));
    assert!(!coordinator.is_animating(settled));
}

#[test]
fn presence_value_tracks_the_business_target() {
    let mut presence = Presence::settled(false, DRAWER_FADE);
    assert!(!presence.value());
    presence.redirect(true, Instant::now());
    assert!(presence.value());
}
