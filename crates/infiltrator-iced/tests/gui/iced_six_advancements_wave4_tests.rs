//! High-fidelity verification tests for Wave 4 of the 6 Iced Core Maturity Advancements.
//!
//! Complies strictly with docs/TEST_GOVERNANCE.md (Zero-Tautology Rule):
//! Every assertion validates concrete business contracts, state transitions,
//! exact string/integer values, and mathematical invariants.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view_root::modals::log_export::tests::exercise_redacted_export;
use infiltrator_contract::network_roaming::{
    NetworkInterfaceKind, NetworkInterfaceSnapshot, NetworkRoamingEvent, NetworkRoamingSnapshot,
    NetworkRoamingStatus,
};
use infiltrator_contract::pac::{PacServiceState, PacSnapshot};
use infiltrator_contract::vpn::VpnSessionSnapshot;
use std::fs::read;

#[test]
fn test_advancement_w4_1_network_roaming_and_gateway_recovery() {
    let (mut state, _) = AppState::new();
    state.shell.demo = true;

    // Initial state
    assert!(state.runtime.network_roaming.interfaces.is_empty());
    assert!(state.runtime.network_roaming.active_interface.is_none());

    // Poll interfaces
    let _ = state.update(Message::PollNetworkInterfaces);
    assert_eq!(state.runtime.network_roaming.interfaces.len(), 2);
    assert_eq!(
        state.runtime.network_roaming.active_interface.as_deref(),
        Some("eth0")
    );
    assert_eq!(
        state.runtime.network_roaming.default_gateway.as_deref(),
        Some("192.168.1.1")
    );
    assert_eq!(
        state.runtime.network_roaming.recommended_tun_mtu,
        Some(1420)
    );

    // Force gateway reconnect
    let _ = state.update(Message::ForceGatewayReconnect);
    assert_eq!(state.runtime.network_roaming.route_repair_count, 1);
    assert!(matches!(
        state.runtime.network_roaming.last_event,
        Some(NetworkRoamingEvent::RoutesRepaired { .. })
    ));
}

#[test]
fn test_live_network_roaming_snapshot_updates_the_iced_projection_without_fallbacks() {
    let (mut state, _) = AppState::new();
    let snapshot = NetworkRoamingSnapshot {
        status: NetworkRoamingStatus::Stable,
        interfaces: vec![NetworkInterfaceSnapshot {
            name: "wlan0".to_owned(),
            kind: NetworkInterfaceKind::Wifi,
            is_up: true,
            is_default_gateway: true,
            gateway_ip: Some("198.51.100.1".to_owned()),
            ip_addresses: vec!["198.51.100.20/24".to_owned()],
            mtu: Some(1400),
            metric: Some(200),
            dns_servers: Vec::new(),
        }],
        active_interface: Some("wlan0".to_owned()),
        default_gateway: Some("198.51.100.1".to_owned()),
        physical_mtu: Some(1400),
        recommended_tun_mtu: Some(1320),
        tcp_mss: Some(1280),
        revision: 8,
        ..Default::default()
    };

    let _ = state.update(Message::NetworkInterfacesPolled(snapshot.clone()));
    assert_eq!(state.runtime.network_roaming, snapshot);
    assert_eq!(
        state.runtime.network_roaming.active_interface.as_deref(),
        Some("wlan0")
    );
    assert_eq!(state.runtime.network_roaming.physical_mtu, Some(1400));
}

#[test]
fn test_vpn_session_snapshot_updates_the_iced_projection() {
    let (mut state, _) = AppState::new();
    let snapshot = VpnSessionSnapshot::running(4, 1500, 2, vec!["1.1.1.1".to_owned()], true, true);
    let _ = state.update(Message::VpnSessionUpdated(Ok(snapshot.clone())));
    assert_eq!(state.runtime.vpn, snapshot);
    assert!(state.runtime.vpn.is_running());
}

#[test]
fn test_advancement_w4_2_crash_watchdog_and_forensics_lifecycle() {
    let (mut state, _) = AppState::new();

    // Initial state
    assert!(!state.diag.crash_watchdog.is_orphaned_detected);
    assert!(state.diag.crash_watchdog.last_crash_summary.is_none());

    // Check watchdog
    let _ = state.update(Message::CheckCrashWatchdog);
    assert_eq!(
        state.diag.crash_watchdog.last_crash_summary.as_deref(),
        Some("No crashes detected in current session")
    );

    // Recover orphaned state
    let _ = state.update(Message::RecoverOrphanedState);
    assert_eq!(
        state.diag.crash_watchdog.recovery_status.as_deref(),
        Some("Orphaned states cleared")
    );

    // Export crash diagnostics
    let _ = state.update(Message::ExportCrashDiagnostics);
    assert_eq!(
        state.diag.crash_watchdog.exported_log_path.as_deref(),
        Some("/tmp/infiltrator_crash_diagnostics.json")
    );

    let json_bytes =
        read("/tmp/infiltrator_crash_diagnostics.json").expect("Diagnostics JSON must exist");
    assert!(!json_bytes.is_empty());
}

#[test]
fn test_advancement_w4_3_web_dashboard_launch_dispatch() {
    let (mut state, _) = AppState::new();

    // Verify launching web dashboards returns valid task without panicking
    let _ = state.update(Message::LaunchWebDashboard("metacubexd"));
    let _ = state.update(Message::LaunchWebDashboard("yacd"));
    let _ = state.update(Message::LaunchWebDashboard("razord"));
}

#[test]
fn test_advancement_w4_4_log_regex_and_redacted_export() {
    let (mut state, _) = AppState::new();

    // Default state
    assert!(state.diag.log_filter.regex_query.is_empty());

    // Update regex query and log level
    let _ = state.update(Message::UpdateLogRegexFilter("error|warn".to_string()));
    assert_eq!(state.diag.log_filter.regex_query, "error|warn");

    let _ = state.update(Message::SetLogLevelFilter("WARN".to_string()));
    assert_eq!(state.diag.log_filter.level_filter, "WARN");

    exercise_redacted_export(&mut state);
}

#[test]
fn test_advancement_w4_5_subscription_quota_and_cron_matrix() {
    use infiltrator_application::subscription_quota_projection::project_quota;
    use infiltrator_contract::subscription_quota::{
        SubscriptionQuotaSnapshot, SubscriptionQuotaStatus,
    };
    let (mut state, _) = AppState::new();
    assert_eq!(
        state.runtime.subscription_quota.status,
        SubscriptionQuotaStatus::Unknown
    );
    let unknown = project_quota(&state.runtime.subscription_quota, "en-US");
    assert_eq!(unknown.used, "—");
    assert_eq!(unknown.total, "—");
    assert_eq!(unknown.fraction, None);
    state.runtime.subscription_quota = SubscriptionQuotaSnapshot {
        status: SubscriptionQuotaStatus::Ready,
        profile_name: Some("provider {usage}".into()),
        used_bytes: Some(0),
        total_bytes: Some(100),
        remaining_bytes: Some(100),
        usage_percent: Some(0.0),
        remaining_percent: Some(100.0),
        ..Default::default()
    };
    assert_eq!(
        state
            .update(Message::UpdateSubscriptionInterval("12".into()))
            .units(),
        0
    );
    assert_eq!(state.profile.subscription_update_interval_hours, "12");
    assert_eq!(state.runtime.subscription_quota.used_bytes, Some(0));
    assert_eq!(state.runtime.subscription_quota.total_bytes, Some(100));
    assert_eq!(
        project_quota(&state.runtime.subscription_quota, "en-US").usage,
        "0.0%"
    );
    assert_eq!(
        state
            .update(Message::UpdateSubscriptionInterval("6".into()))
            .units(),
        0
    );
    assert_eq!(state.profile.subscription_update_interval_hours, "6");
    assert_eq!(
        project_quota(&state.runtime.subscription_quota, "zh-CN").profile,
        "provider {usage}"
    );
}

#[test]
fn test_advancement_w4_6_pac_auto_proxy_and_bypass_manager() {
    let (mut state, _) = AppState::new();
    state.shell.demo = true;

    // Default PAC state
    assert!(!state.runtime.pac_manager.is_pac_mode_active);
    assert!(state.runtime.pac_manager.pac_url.is_empty());

    // Update bypass subnets
    let bypass = "localhost; 127.*; 192.168.*; 10.*";
    let _ = state.update(Message::UpdatePacBypassSubnets(bypass.to_string()));
    assert_eq!(state.runtime.pac_manager.bypass_subnets, bypass);

    // Toggle PAC mode on
    let _ = state.update(Message::TogglePacMode(true));
    assert!(state.runtime.pac_manager.is_pac_mode_active);
    assert_eq!(
        state.runtime.pac_manager.pac_url,
        "http://127.0.0.1:25211/proxy.pac"
    );

    // Compile and validate PAC
    let _ = state.update(Message::CompileAndValidatePac);
    assert_eq!(
        state.runtime.pac_manager.last_compile_status.as_deref(),
        Some("Valid PAC compiled")
    );

    // Toggle PAC mode off
    let _ = state.update(Message::TogglePacMode(false));
    assert!(!state.runtime.pac_manager.is_pac_mode_active);
    assert!(state.runtime.pac_manager.pac_url.is_empty());
}

#[test]
fn test_pac_live_result_updates_url_and_committed_snapshot() {
    let (mut state, _) = AppState::new();
    state.runtime.pac_manager.bypass_subnets = "old.example".to_owned();
    state.runtime.pac_manager.dirty = true;
    let snapshot = PacSnapshot {
        state: PacServiceState::Running {
            url: "http://127.0.0.1:32000/proxy.pac".to_owned(),
        },
        script_bytes: 4096,
        bypass_domains: vec!["example.com".to_owned()],
        revision: 5,
    };

    let _ = state.update(Message::PacApplied(Ok(snapshot)));

    assert!(state.runtime.pac_manager.is_pac_mode_active);
    assert_eq!(
        state.runtime.pac_manager.pac_url,
        "http://127.0.0.1:32000/proxy.pac"
    );
    assert_eq!(state.runtime.pac_manager.bypass_subnets, "example.com");
    assert_eq!(state.runtime.pac_manager.snapshot.script_bytes, 4096);
    assert!(!state.runtime.pac_manager.dirty);
}
