//! Behavior cases for service.
//! test-intent: behavior

use super::*;

#[test]
fn test_service_status_equality() {
    let status1 = ServiceStatus::Running(1234);
    let status2 = ServiceStatus::Running(1234);
    let status3 = ServiceStatus::Running(5678);
    let status4 = ServiceStatus::Stopped;

    assert_eq!(status1, status2);
    assert_ne!(status1, status3);
    assert_ne!(status1, status4);
}

#[test]
fn test_service_manager_new() {
    let temp_dir = TempDir::new().unwrap();
    let binary_path = temp_dir.path().join("mihomo.exe");
    let config_path = temp_dir.path().join("config.yaml");
    let manager = ServiceManager::new(binary_path.clone(), config_path.clone());
    assert_eq!(manager.binary_path(), binary_path.as_path());
}

#[test]
fn test_service_manager_with_home() {
    let temp_dir = TempDir::new().unwrap();
    let binary_path = temp_dir.path().join("mihomo.exe");
    let config_path = temp_dir.path().join("config.yaml");
    let home = temp_dir.path().join("home");
    let manager = ServiceManager::with_home(binary_path.clone(), config_path.clone(), home);
    assert_eq!(manager.binary_path(), binary_path.as_path());
}

#[test]
fn test_service_manager_with_pid_file() {
    let temp_dir = TempDir::new().unwrap();
    let binary_path = temp_dir.path().join("mihomo.exe");
    let config_path = temp_dir.path().join("config.yaml");
    let pid_file = temp_dir.path().join("pidfile");
    let manager = ServiceManager::with_pid_file(binary_path.clone(), config_path.clone(), pid_file);
    assert_eq!(manager.binary_path(), binary_path.as_path());
}

#[test]
fn test_service_status_debug() {
    let running = ServiceStatus::Running(1234);
    let debug_str = format!("{:?}", running);
    assert!(debug_str.contains("Running"));
    assert!(debug_str.contains("1234"));

    let stopped = ServiceStatus::Stopped;
    assert_eq!(format!("{:?}", stopped), "Stopped");
}

#[test]
fn test_service_state_display_and_running() {
    assert_eq!(ServiceState::NotInstalled.to_string(), "Not Installed");
    assert_eq!(ServiceState::Stopped.to_string(), "Stopped");
    assert_eq!(ServiceState::Running.to_string(), "Running");
    assert_eq!(
        ServiceState::Error("Daemon crashed".to_string()).to_string(),
        "Error: Daemon crashed"
    );

    assert!(ServiceState::Running.is_running());
    assert!(!ServiceState::Stopped.is_running());
    assert!(!ServiceState::NotInstalled.is_running());
    assert!(!ServiceState::Error("fail".to_string()).is_running());
}

#[test]
fn test_service_status_info_fallback() {
    let uninstalled = ServiceStatusInfo::fallback_uninstalled();
    assert_eq!(uninstalled.state, ServiceState::NotInstalled);
    assert!(!uninstalled.tun_active);
    assert!(!uninstalled.system_proxy_active);
    assert_eq!(uninstalled.version, SERVICE_VERSION);

    let stopped = ServiceStatusInfo::fallback_stopped();
    assert_eq!(stopped.state, ServiceState::Stopped);
    assert!(!stopped.tun_active);
    assert!(!stopped.system_proxy_active);
}

#[test]
fn test_service_error_display() {
    assert_eq!(
        ServiceError::NotInstalled.to_string(),
        "Service is not installed"
    );
    assert_eq!(
        ServiceError::NotRunning.to_string(),
        "Service is not running"
    );
    assert!(
        ServiceError::ConnectionFailed("refused".to_string())
            .to_string()
            .contains("refused")
    );
    assert!(
        ServiceError::Unauthorized("bad token".to_string())
            .to_string()
            .contains("bad token")
    );
    assert!(
        ServiceError::ProtocolError("bad json".to_string())
            .to_string()
            .contains("bad json")
    );
    assert!(
        ServiceError::CommandFailed("driver error".to_string())
            .to_string()
            .contains("driver error")
    );
    assert_eq!(
        ServiceError::Timeout.to_string(),
        "Service communication timed out"
    );
    assert!(
        ServiceError::Io("disk error".to_string())
            .to_string()
            .contains("disk error")
    );
}

#[test]
fn test_service_state_machine_full_lifecycle() {
    use super::state_machine::{LifecycleEvent, LifecycleState, ServiceStateMachine};

    let mut sm = ServiceStateMachine::new(LifecycleState::Uninstalled).with_max_history(50);
    assert_eq!(sm.current_state(), &LifecycleState::Uninstalled);
    assert!(!sm.is_running());
    assert!(!sm.is_installed());

    // 1. Installation: Uninstalled -> Installing -> InstalledStopped
    assert!(sm.can_apply(&LifecycleEvent::InstallStart));
    sm.apply(LifecycleEvent::InstallStart).unwrap();
    assert_eq!(sm.current_state(), &LifecycleState::Installing);
    assert!(!sm.is_installed());

    sm.apply(LifecycleEvent::InstallSuccess).unwrap();
    assert_eq!(sm.current_state(), &LifecycleState::InstalledStopped);
    assert!(sm.is_installed());
    assert!(!sm.is_running());

    // 2. Start Service: InstalledStopped -> Starting -> Running
    sm.apply(LifecycleEvent::StartRequested).unwrap();
    assert_eq!(sm.current_state(), &LifecycleState::Starting);

    sm.apply(LifecycleEvent::StartSuccess {
        pid: Some(9999),
        tun_active: false,
    })
    .unwrap();
    assert_eq!(
        sm.current_state(),
        &LifecycleState::Running {
            pid: Some(9999),
            tun_active: false,
            proxy_active: false,
        }
    );
    assert!(sm.is_running());
    assert_eq!(sm.current_state().pid(), Some(9999));
    assert!(!sm.current_state().is_tun_active());
    assert!(!sm.current_state().is_proxy_active());

    // 3. Routing commands: StartTun, ProxyApplied, ProxyCleared, TunStopped
    sm.apply(LifecycleEvent::TunStarted {
        interface_name: Some("tun0".to_string()),
    })
    .unwrap();
    assert!(sm.current_state().is_tun_active());

    sm.apply(LifecycleEvent::ProxyApplied).unwrap();
    assert!(sm.current_state().is_proxy_active());

    sm.apply(LifecycleEvent::ProxyCleared).unwrap();
    assert!(!sm.current_state().is_proxy_active());

    sm.apply(LifecycleEvent::TunStopped).unwrap();
    assert!(!sm.current_state().is_tun_active());

    // 4. Stop Service: Running -> Stopping -> InstalledStopped
    sm.apply(LifecycleEvent::StopRequested).unwrap();
    assert_eq!(sm.current_state(), &LifecycleState::Stopping);

    sm.apply(LifecycleEvent::StopSuccess).unwrap();
    assert_eq!(sm.current_state(), &LifecycleState::InstalledStopped);
    assert!(!sm.is_running());

    // 5. Uninstallation: InstalledStopped -> Uninstalling -> Uninstalled
    sm.apply(LifecycleEvent::UninstallStart).unwrap();
    assert_eq!(sm.current_state(), &LifecycleState::Uninstalling);

    sm.apply(LifecycleEvent::UninstallSuccess).unwrap();
    assert_eq!(sm.current_state(), &LifecycleState::Uninstalled);
    assert!(!sm.is_installed());

    // Check transition history
    assert!(sm.history().len() >= 10);
}

#[test]
fn test_service_state_machine_error_and_degraded_recovery() {
    use super::state_machine::{LifecycleEvent, LifecycleState, ServiceStateMachine};

    // 1. Install failure recovery
    let mut sm = ServiceStateMachine::new(LifecycleState::Uninstalled);
    sm.apply(LifecycleEvent::InstallStart).unwrap();
    sm.apply(LifecycleEvent::InstallFailure(
        "Permission denied".to_string(),
    ))
    .unwrap();
    assert!(matches!(
        sm.current_state(),
        LifecycleState::Error {
            recoverable: true,
            ..
        }
    ));
    sm.apply(LifecycleEvent::Recover).unwrap();
    assert_eq!(sm.current_state(), &LifecycleState::InstalledStopped);

    // 2. Startup failure recovery
    sm.apply(LifecycleEvent::StartRequested).unwrap();
    sm.apply(LifecycleEvent::StartFailure("Port conflict".to_string()))
        .unwrap();
    assert!(matches!(
        sm.current_state(),
        LifecycleState::Error {
            recoverable: true,
            ..
        }
    ));
    sm.apply(LifecycleEvent::Recover).unwrap();
    assert_eq!(sm.current_state(), &LifecycleState::InstalledStopped);

    // 3. Degraded state & recovery
    sm.apply(LifecycleEvent::StartSuccess {
        pid: Some(1234),
        tun_active: true,
    })
    .unwrap();
    sm.apply(LifecycleEvent::Degrade("TUN route packet loss".to_string()))
        .unwrap();
    assert!(matches!(
        sm.current_state(),
        LifecycleState::Degraded { .. }
    ));
    assert!(sm.current_state().is_tun_active());
    sm.apply(LifecycleEvent::Recover).unwrap();
    assert!(sm.is_running());

    // 4. Heartbeat missed -> Degraded -> Stop -> Stopped
    sm.apply(LifecycleEvent::HeartbeatMissed).unwrap();
    assert!(matches!(
        sm.current_state(),
        LifecycleState::Degraded { .. }
    ));
    sm.apply(LifecycleEvent::StopRequested).unwrap();
    sm.apply(LifecycleEvent::StopSuccess).unwrap();
    assert_eq!(sm.current_state(), &LifecycleState::InstalledStopped);

    // 5. Crash recovery
    sm.apply(LifecycleEvent::StartRequested).unwrap();
    sm.apply(LifecycleEvent::ProcessCrashed("SIGKILL".to_string()))
        .unwrap();
    assert!(matches!(sm.current_state(), LifecycleState::Error { .. }));
    sm.apply(LifecycleEvent::Reset).unwrap();
    assert_eq!(sm.current_state(), &LifecycleState::InstalledStopped);

    // 6. Stop failure & Uninstall failure
    sm.apply(LifecycleEvent::StartSuccess {
        pid: Some(555),
        tun_active: false,
    })
    .unwrap();
    sm.apply(LifecycleEvent::StopRequested).unwrap();
    sm.apply(LifecycleEvent::StopFailure("Timeout".to_string()))
        .unwrap();
    assert!(matches!(sm.current_state(), LifecycleState::Error { .. }));
    sm.apply(LifecycleEvent::Reset).unwrap();

    sm.apply(LifecycleEvent::UninstallStart).unwrap();
    sm.apply(LifecycleEvent::UninstallFailure("File locked".to_string()))
        .unwrap();
    assert!(matches!(sm.current_state(), LifecycleState::Error { .. }));
}

#[test]
fn test_service_state_machine_illegal_transitions() {
    use super::state_machine::{LifecycleEvent, LifecycleState, ServiceStateMachine};

    let mut sm = ServiceStateMachine::new(LifecycleState::Uninstalled);

    // Cannot start or stop when uninstalled
    assert!(!sm.can_apply(&LifecycleEvent::StartRequested));
    let err = sm.apply(LifecycleEvent::StartRequested).unwrap_err();
    assert!(err.to_string().contains("uninstalled"));

    assert!(!sm.can_apply(&LifecycleEvent::TunStarted {
        interface_name: None
    }));
    let err2 = sm
        .apply(LifecycleEvent::TunStarted {
            interface_name: None,
        })
        .unwrap_err();
    assert!(err2.to_string().contains("uninstalled"));

    // Transition to InstalledStopped
    sm.apply(LifecycleEvent::InstallSuccess).unwrap();

    // Cannot route commands when stopped
    let err3 = sm.apply(LifecycleEvent::ProxyApplied).unwrap_err();
    assert!(err3.to_string().contains("stopped"));

    // Transition to Installing
    sm.apply(LifecycleEvent::InstallStart).unwrap();
    let err4 = sm.apply(LifecycleEvent::StartRequested).unwrap_err();
    assert!(err4.to_string().contains("in progress"));
}
