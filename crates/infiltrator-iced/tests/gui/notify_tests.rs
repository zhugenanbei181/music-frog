use super::*;
use std::env::{remove_var, set_var};

#[cfg(all(unix, not(target_os = "macos")))]
#[test]
fn daemon_urgency_never_persists_and_timeout_is_bounded() {
    // Regression guard: the daemon `critical` urgency is defined as
    // "must be acknowledged" and is rendered persistent by KDE Plasma, which
    // pinned notifications on screen forever. It must never be emitted, and
    // every urgency must carry a positive bounded display timeout.
    assert_eq!(
        backend::to_daemon_urgency(NotifyUrgency::Low),
        notify_rust::Urgency::Low
    );
    assert_eq!(
        backend::to_daemon_urgency(NotifyUrgency::Normal),
        notify_rust::Urgency::Normal
    );
    assert_eq!(
        backend::to_daemon_urgency(NotifyUrgency::Critical),
        notify_rust::Urgency::Normal
    );

    for urgency in [
        NotifyUrgency::Low,
        NotifyUrgency::Normal,
        NotifyUrgency::Critical,
    ] {
        let timeout = NotificationLevel::from(urgency).display_timeout_ms();
        assert!(timeout > 0, "notification must expire");
        assert!(timeout <= 10_000, "notification must stay bounded");
    }
}

#[test]
fn force_notify_requested_reflects_environment() {
    // When unset or set to non-1 value
    unsafe {
        remove_var("INFILTRATOR_FORCE_NOTIFY");
    }
    assert!(!force_notify_requested());

    unsafe {
        set_var("INFILTRATOR_FORCE_NOTIFY", "0");
    }
    assert!(!force_notify_requested());

    unsafe {
        set_var("INFILTRATOR_FORCE_NOTIFY", "1");
    }
    assert!(force_notify_requested());

    unsafe {
        remove_var("INFILTRATOR_FORCE_NOTIFY");
    }
}
