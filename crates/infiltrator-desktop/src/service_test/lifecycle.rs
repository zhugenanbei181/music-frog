//! Behavior cases for lifecycle.
//! test-intent: behavior

#[test]
fn test_lifecycle_state_and_event_display() {
    use super::state_machine::LifecycleState;

    assert_eq!(LifecycleState::Uninstalled.to_string(), "Uninstalled");
    assert_eq!(LifecycleState::Installing.to_string(), "Installing");
    assert_eq!(
        LifecycleState::InstalledStopped.to_string(),
        "Installed (Stopped)"
    );
    assert_eq!(LifecycleState::Starting.to_string(), "Starting");
    assert_eq!(LifecycleState::Stopping.to_string(), "Stopping");
    assert_eq!(LifecycleState::Uninstalling.to_string(), "Uninstalling");

    let running_str = LifecycleState::Running {
        pid: Some(123),
        tun_active: true,
        proxy_active: false,
    }
    .to_string();
    assert!(running_str.contains("Running"));
    assert!(running_str.contains("pid=Some(123)"));

    let degraded_str = LifecycleState::Degraded {
        reason: "NIC issue".to_string(),
        tun_active: false,
        proxy_active: true,
    }
    .to_string();
    assert!(degraded_str.contains("Degraded: NIC issue"));

    let error_str = LifecycleState::Error {
        message: "Crash".to_string(),
        recoverable: true,
    }
    .to_string();
    assert!(error_str.contains("Error (recoverable=true): Crash"));
}
