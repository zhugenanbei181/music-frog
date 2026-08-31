//! Admin server lifecycle glue tests (mounted from `src/admin_server.rs`):
//! plan_admin_server_action transitions, manager bookkeeping, dev-checkout
//! resolution and the config-flag reader. Under cfg(test) no real server
//! ever starts and dropped Tasks never persist settings to disk.

use super::*;

#[test]
fn plan_starts_when_enabled_and_not_running() {
    let desired = AdminServerConfig::default();
    assert_eq!(
        plan_admin_server_action(false, None, desired),
        AdminServerIntent::Start
    );
}

#[test]
fn plan_stops_when_disabled_and_running() {
    let desired = AdminServerConfig {
        enabled: false,
        port: ADMIN_DEFAULT_PORT,
    };
    let running_config = AdminServerConfig::default();
    assert_eq!(
        plan_admin_server_action(true, Some(running_config), desired),
        AdminServerIntent::Stop
    );
}

#[test]
fn plan_is_noop_when_disabled_and_not_running() {
    let desired = AdminServerConfig {
        enabled: false,
        port: ADMIN_DEFAULT_PORT,
    };
    assert_eq!(
        plan_admin_server_action(false, None, desired),
        AdminServerIntent::None
    );
}

#[test]
fn plan_is_noop_when_running_with_same_config() {
    let desired = AdminServerConfig {
        enabled: true,
        port: 25300,
    };
    assert_eq!(
        plan_admin_server_action(true, Some(desired.clone()), desired),
        AdminServerIntent::None
    );
}

#[test]
fn plan_restarts_on_port_change() {
    let running_config = AdminServerConfig {
        enabled: true,
        port: 25210,
    };
    let desired = AdminServerConfig {
        enabled: true,
        port: 25300,
    };
    assert_eq!(
        plan_admin_server_action(true, Some(running_config), desired),
        AdminServerIntent::Restart
    );
}

#[test]
fn plan_restarts_when_running_without_bookkeeping() {
    let desired = AdminServerConfig::default();
    assert_eq!(
        plan_admin_server_action(true, None, desired),
        AdminServerIntent::Restart
    );
}

#[test]
fn manager_start_then_stop_transition() {
    let manager = AdminServerManager::new();
    let desired = AdminServerConfig::default();

    assert_eq!(
        manager.begin_transition(desired.clone()),
        AdminServerIntent::Start,
        "settings toggle should produce a start intent"
    );
    assert_eq!(manager.started_config(), Some(desired.clone()));
    assert!(!manager.is_running(), "no real server may start in tests");

    assert_eq!(
        manager.begin_transition(AdminServerConfig {
            enabled: false,
            port: desired.port,
        }),
        AdminServerIntent::Stop,
        "disabling should produce a stop intent"
    );
    assert_eq!(manager.started_config(), None);
    assert!(!manager.is_running());
}

#[test]
fn manager_port_change_while_pending_is_a_restart() {
    let manager = AdminServerManager::new();
    let first = AdminServerConfig {
        enabled: true,
        port: 25210,
    };
    assert_eq!(manager.begin_transition(first), AdminServerIntent::Start);
    // No real server ran (test build), but the start is bookkept as
    // pending, so a new port supersedes it as a restart.
    assert_eq!(
        manager.begin_transition(AdminServerConfig {
            enabled: true,
            port: 25300,
        }),
        AdminServerIntent::Restart,
        "a pending start superseded by a new port is a restart"
    );
}

#[test]
fn manager_shutdown_is_idempotent() {
    let manager = AdminServerManager::new();
    let _ = manager.begin_transition(AdminServerConfig::default());
    manager.shutdown();
    manager.shutdown();
    assert!(!manager.is_running());
    assert_eq!(manager.started_config(), None);
    assert!(manager.url().is_none());
}

#[test]
fn admin_enabled_reads_config_flag() {
    assert!(admin_enabled(&AdminServerConfig::default()));
    assert!(!admin_enabled(&AdminServerConfig {
        enabled: false,
        port: 1
    }));
}
