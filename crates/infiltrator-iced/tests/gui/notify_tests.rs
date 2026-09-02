use super::*;

#[test]
fn warn_throttled_first_call_then_throttled() {
    warn_throttled("notify tests: first");
    warn_throttled("notify tests: throttled within window");
}

#[test]
fn urgency_is_copy_and_comparable() {
    let low = NotifyUrgency::Low;
    let copy = low;
    assert_eq!(copy, NotifyUrgency::Low);
    assert_ne!(low, NotifyUrgency::Critical);
}

#[test]
fn urgency_to_notification_level_conversion() {
    assert_eq!(
        NotificationLevel::from(NotifyUrgency::Low),
        NotificationLevel::Info
    );
    assert_eq!(
        NotificationLevel::from(NotifyUrgency::Normal),
        NotificationLevel::Info
    );
    assert_eq!(
        NotificationLevel::from(NotifyUrgency::Critical),
        NotificationLevel::Error
    );
}

#[test]
fn smoke_probe_title_is_valid() {
    assert_eq!(SMOKE_PROBE_TITLE, "infiltrator-notify-probe");
}

#[test]
fn force_notify_requested_reflects_environment() {
    // When unset or set to non-1 value
    unsafe {
        std::env::remove_var("INFILTRATOR_FORCE_NOTIFY");
    }
    assert!(!force_notify_requested());

    unsafe {
        std::env::set_var("INFILTRATOR_FORCE_NOTIFY", "0");
    }
    assert!(!force_notify_requested());

    unsafe {
        std::env::set_var("INFILTRATOR_FORCE_NOTIFY", "1");
    }
    assert!(force_notify_requested());

    unsafe {
        std::env::remove_var("INFILTRATOR_FORCE_NOTIFY");
    }
}
