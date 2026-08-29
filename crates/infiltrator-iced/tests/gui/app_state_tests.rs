//! Core AppState pipeline tests: navigation, runtime/config/status flows,
//! DNS & profile loading, editor, tray glue, tabs and i18n fallback.
//! Mounted via `src/test_mounts.rs` (crate root).
//! test-intent: behavior

use crate::locales::{Lang, Localizer};
use crate::types::{
    DnsTab, RebuildFlowState, RulesJsonTab, RulesTab, RuntimeConfig,
};
use crate::{AppState, InfiltratorError, Message, Route, RuntimeStatus};
use iced::widget::text_editor;
use infiltrator_core::rules::RuleEntry;
use mihomo_api::types::TrafficData;
use mihomo_config::profile::Profile;
use std::path::PathBuf;

#[test]
fn test_route_navigation() {
    let (mut state, _) = AppState::new();
    assert_eq!(state.current_route, Route::Overview);

    let _ = state.update(Message::Navigate(Route::Runtime));
    assert_eq!(state.current_route, Route::Runtime);

    let _ = state.update(Message::Navigate(Route::Settings));
    assert_eq!(state.current_route, Route::Settings);
}

#[test]
fn test_runtime_config_sync() {
    let (mut state, _) = AppState::new();

    // Simulate config fetch
    let _ = state.update(Message::RuntimeConfigFetched(Ok(RuntimeConfig {
        mode: "global".into(),
        tun_enabled: true,
        dns_nameservers: vec!["1.1.1.1".into()],
        dns_fallback: vec!["8.8.8.8".into()],
        dns_enhanced_mode: "fake-ip".into(),
        tun_stack: "gvisor".into(),
        tun_auto_route: true,
        tun_strict_route: false,
        sniffer_enabled: true,
    })));

    assert_eq!(state.proxy_mode.as_ref().unwrap(), "global");
    assert!(state.tun_enabled.unwrap());
    assert_eq!(state.dns_nameservers[0], "1.1.1.1");
}

#[test]
fn test_mode_set_interactions() {
    let (mut state, _) = AppState::new();

    // Success path (should trigger a re-fetch)
    let _task = state.update(Message::ModeSetResult(Ok(())));
    assert!(state.error_msg.is_none());

    // Failure path
    let _ = state.update(Message::ModeSetResult(Err(InfiltratorError::Mihomo(
        "API Error".into(),
    ))));
    assert_eq!(
        state.error_msg.as_ref().unwrap(),
        "Mihomo API error: API Error"
    );
}

#[test]
fn test_traffic_throttling_logic() {
    let (mut state, _) = AppState::new();
    let _ = state.update(Message::TrafficReceived(TrafficData {
        up: 1000,
        down: 1000,
    }));
    assert_eq!(state.traffic.as_ref().unwrap().up, 1000);

    // No throttling currently implemented
    let _ = state.update(Message::TrafficReceived(TrafficData {
        up: 1500,
        down: 1500,
    }));
    assert_eq!(state.traffic.as_ref().unwrap().up, 1500);

    // Updated
    let _ = state.update(Message::TrafficReceived(TrafficData {
        up: 3000,
        down: 3000,
    }));
    assert_eq!(state.traffic.as_ref().unwrap().up, 3000);
}

#[test]
fn test_dns_server_list_manipulation() {
    let (mut state, _) = AppState::new();
    state.dns_nameservers = vec!["old".into()];

    let _ = state.update(Message::UpdateDnsServer(0, "new".into()));
    assert_eq!(state.dns_nameservers[0], "new");

    let _ = state.update(Message::AddDnsServer);
    assert_eq!(state.dns_nameservers.len(), 2);

    let _ = state.update(Message::AddDnsServerTemplate(
        "https://1.1.1.1/dns-query".into(),
    ));
    assert_eq!(state.dns_nameservers.len(), 3);
    assert_eq!(state.dns_nameservers[2], "https://1.1.1.1/dns-query");

    let _ = state.update(Message::RemoveDnsServer(0));
    assert_eq!(state.dns_nameservers.len(), 2);
    assert_eq!(state.dns_nameservers[0], "");

    // Fallbacks
    let _ = state.update(Message::AddFallbackDnsServer);
    assert_eq!(state.dns_fallback_servers.len(), 1);

    let _ = state.update(Message::UpdateFallbackDnsServer(0, "8.8.8.8".into()));
    assert_eq!(state.dns_fallback_servers[0], "8.8.8.8");

    let _ = state.update(Message::RemoveFallbackDnsServer(0));
    assert_eq!(state.dns_fallback_servers.len(), 0);
}

#[test]
fn test_system_integration_states() {
    let (mut state, _) = AppState::new();

    // System Proxy
    state.system_proxy_enabled = false;
    let _ = state.update(Message::SetSystemProxy(true));
    assert!(state.system_proxy_enabled);

    // Rollback on error
    let _ = state.update(Message::SystemProxySet(Err(InfiltratorError::Privilege(
        "Access denied".into(),
    ))));
    assert!(!state.system_proxy_enabled, "Should rollback on failure");
    assert_eq!(
        state.error_msg.as_ref().unwrap(),
        "Privilege error: Access denied"
    );

    // Autostart
    state.autostart_enabled = false;
    let _ = state.update(Message::SetAutostart(true));
    assert!(state.autostart_enabled);

    let _ = state.update(Message::AutostartSet(Err(InfiltratorError::Internal(
        "Registry lock".into(),
    ))));
    assert!(
        !state.autostart_enabled,
        "Should rollback autostart on failure"
    );
}

#[test]
fn test_profiles_and_rules_loading() {
    let (mut state, _) = AppState::new();

    // Profiles loaded
    let _ = state.update(Message::ProfilesLoaded(Ok(vec![Profile::new(
        "test".into(),
        PathBuf::from("test.yaml"),
        true,
    )])));
    assert_eq!(state.profiles.len(), 1);
    assert!(!state.is_loading_profiles);

    // Rules loaded
    let _ = state.update(Message::RulesLoaded(Ok(vec![RuleEntry {
        rule: "DOMAIN,example.com,DIRECT".into(),
        enabled: true,
    }])));
    assert_eq!(state.rules.len(), 1);
}

#[test]
fn test_proxy_lifecycle_messages() {
    let (mut state, _) = AppState::new();

    let _ = state.update(Message::StartProxy);
    assert_eq!(state.status, RuntimeStatus::Starting);

    let _ = state.update(Message::ProxyStopped);
    assert!(state.traffic.is_none());
}

#[test]
fn test_rebuild_flow_state_transitions() {
    let (mut state, _) = AppState::new();
    state.rules = vec![RuleEntry {
        rule: "MATCH,DIRECT".into(),
        enabled: true,
    }];

    let _ = state.update(Message::SaveRules);
    assert!(matches!(
        state.rebuild_flow,
        RebuildFlowState::Saving { .. }
    ));

    let _ = state.update(Message::RuntimeRebuildFinished(Err(
        InfiltratorError::Mihomo("boom".into()),
    )));
    assert!(matches!(
        state.rebuild_flow,
        RebuildFlowState::Failed { .. }
    ));

    let _ = state.update(Message::ClearRebuildFlow);
    assert!(matches!(state.rebuild_flow, RebuildFlowState::Idle));
}

#[test]
fn test_log_buffer_limit_and_queue() {
    let (mut state, _) = AppState::new();

    for i in 0..650 {
        let _ = state.update(Message::LogReceived(format!("log {}", i)));
    }

    assert_eq!(state.logs.len(), 500);
    assert_eq!(state.logs.front().unwrap(), "log 150");
    assert_eq!(state.logs.back().unwrap(), "log 649");
}

#[test]
fn test_editor_actions() {
    let (mut state, _) = AppState::new();

    // Simulate successful load
    let _ = state.update(Message::ProfileContentLoaded(Ok((
        PathBuf::from("config.yaml"),
        "proxies: []".into(),
    ))));
    assert_eq!(
        state.editor_path.as_ref().unwrap().to_str().unwrap(),
        "config.yaml"
    );
    assert_eq!(state.editor_content.text(), "proxies: []");

    // Editor action (simulating typing)
    let _ = state.update(Message::EditorAction(text_editor::Action::Edit(
        text_editor::Edit::Insert('a'),
    )));
    assert_ne!(state.editor_content.text(), "proxies: []");

    // Save success
    state.current_route = Route::Editor;
    let _ = state.update(Message::ProfileSaved(Ok(())));
    assert_eq!(state.current_route, Route::Editor);
}

#[test]
fn test_tray_and_exit() {
    let (mut state, _) = AppState::new();

    // Tray events shouldn't crash and must map onto the same actions as the
    // old muda menu ids: icon click shows the window, quit requests exit.
    let _ = state.update(Message::TrayEvent(crate::tray::TrayEvent::IconActivated));
    assert_eq!(
        crate::tray::resolve_tray_event(
            &crate::tray::TrayEvent::MenuActivated {
                id: crate::tray::TRAY_ACTION_QUIT,
                payload: None,
            },
            false,
            false,
        ),
        Some(crate::tray::TrayIntent::Exit)
    );
    let _ = state.update(Message::TrayEvent(crate::tray::TrayEvent::MenuActivated {
        id: crate::tray::TRAY_ACTION_QUIT,
        payload: None,
    }));

    let _ = state.update(Message::Exit);
}

#[test]
fn test_tab_state_switches() {
    let (mut state, _) = AppState::new();
    state.rules_page = 3;
    let _ = state.update(Message::SetRulesTab(RulesTab::JsonEditors));
    assert_eq!(state.rules_tab, RulesTab::JsonEditors);
    assert_eq!(state.rules_page, 0);

    let _ = state.update(Message::SetRulesJsonTab(RulesJsonTab::Sniffer));
    assert_eq!(state.rules_json_tab, RulesJsonTab::Sniffer);

    let _ = state.update(Message::SetDnsTab(DnsTab::Tun));
    assert_eq!(state.dns_tab, DnsTab::Tun);
}

#[test]
fn test_i18n_fallback() {
    let lang = Lang("fr-FR"); // Unsupported
    assert_eq!(
        lang.tr("nav_overview"),
        "核心概览",
        "Should fallback to ZH for unsupported locales"
    );
}
