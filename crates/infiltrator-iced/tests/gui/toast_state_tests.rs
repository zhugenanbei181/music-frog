//! `toast_state` unit tests (mounted from `src/toast_state.rs`): toast
//! visibility ordering, timer expiry, deduplication and manual dismissal.

use super::*;

#[test]
fn test_push_and_visible_order() {
    let mut manager = ToastManager::new(2);
    manager.push(ToastLevel::Info, "A".to_string(), 5000);
    manager.push(ToastLevel::Info, "B".to_string(), 5000);
    assert_eq!(manager.active_toasts().len(), 2);
    assert_eq!(manager.active_toasts()[0].message, "A");
    assert_eq!(manager.active_toasts()[1].message, "B");

    // Pushing a 3rd should drop the oldest ("A")
    manager.push(ToastLevel::Info, "C".to_string(), 5000);
    assert_eq!(manager.active_toasts().len(), 2);
    assert_eq!(manager.active_toasts()[0].message, "B");
    assert_eq!(manager.active_toasts()[1].message, "C");
}

#[test]
fn test_tick_timer_and_expiration() {
    let mut manager = ToastManager::new(5);
    manager.push(ToastLevel::Success, "A".to_string(), 3000);
    manager.tick(1000);
    assert_eq!(manager.active_toasts()[0].remaining_ms, 2000);
    manager.tick(2000);
    assert!(manager.active_toasts().is_empty());
}

#[test]
fn test_message_deduplication() {
    let mut manager = ToastManager::new(5);
    manager.push(ToastLevel::Error, "A".to_string(), 5000);
    manager.tick(1000);

    manager.push(ToastLevel::Error, "A".to_string(), 5000);
    assert_eq!(manager.active_toasts().len(), 1);
    assert_eq!(manager.active_toasts()[0].remaining_ms, 5000);

    manager.tick(3000);

    manager.push(ToastLevel::Error, "A".to_string(), 5000);
    assert_eq!(manager.active_toasts().len(), 2);
}

#[test]
fn test_manual_dismiss() {
    let mut manager = ToastManager::new(5);
    manager.push(ToastLevel::Warning, "A".to_string(), 5000);
    let id = manager.active_toasts()[0].id;
    manager.dismiss(id);
    assert!(manager.active_toasts().is_empty());
}
