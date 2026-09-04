//! Core AppState pipeline tests: navigation, runtime/config/status flows,
//! DNS & profile loading, editor, tray glue, tabs and i18n fallback.
//! Mounted via `src/test_mounts.rs` (crate root).
//! test-intent: behavior

use crate::state::AppState;
use crate::types::app::{ConfirmAction, CoreDownloadProgress, Route, SyncProgress};
use crate::types::dns::DnsTab;
use crate::types::message::Message;
use crate::types::rules::{RulesJsonTab, RulesTab};
use crate::types::runtime::{
    IpProbeResult, RuntimePatchSnapshot, RuntimeStatus, RuntimeStreamKind, RuntimeStreamState,
};
use crate::types::runtime::{RebuildFlowState, RuntimeConfig};
use iced::widget::text_editor;
use infiltrator_core::error::InfiltratorError;
use infiltrator_core::rules::RuleEntry;
use infiltrator_shared::locales::{Lang, Localizer};
use mihomo_api::types::TrafficData;
use mihomo_config::profile::Profile;
use std::path::PathBuf;

#[test]
fn test_route_navigation() {
    let (mut state, _) = AppState::new();
    assert_eq!(state.shell.current_route, Route::Overview);
    assert!(!state.shell.history.can_go_back());
    assert!(!state.shell.history.can_go_forward());

    let _ = state.update(Message::Navigate(Route::Runtime));
    assert_eq!(state.shell.current_route, Route::Runtime);
    assert!(state.shell.history.can_go_back());
    assert!(!state.shell.history.can_go_forward());

    let _ = state.update(Message::Navigate(Route::Settings));
    assert_eq!(state.shell.current_route, Route::Settings);

    let _ = state.update(Message::Navigate(Route::Doctor));
    assert_eq!(state.shell.current_route, Route::Doctor);

    let _ = state.update(Message::Navigate(Route::AppRouting));
    assert_eq!(state.shell.current_route, Route::AppRouting);

    // Back to Doctor
    let _ = state.update(Message::NavigateBack);
    assert_eq!(state.shell.current_route, Route::Doctor);
    assert!(state.shell.history.can_go_forward());

    // Back to Settings
    let _ = state.update(Message::NavigateBack);
    assert_eq!(state.shell.current_route, Route::Settings);

    // Forward to Doctor
    let _ = state.update(Message::NavigateForward);
    assert_eq!(state.shell.current_route, Route::Doctor);

    // Navigate to Proxies branches and clears forward stack
    let _ = state.update(Message::Navigate(Route::Proxies));
    assert_eq!(state.shell.current_route, Route::Proxies);
    assert!(!state.shell.history.can_go_forward());

    // Same route navigation is idempotent
    let _ = state.update(Message::Navigate(Route::Proxies));
    assert_eq!(state.shell.current_route, Route::Proxies);
}

#[test]
fn test_runtime_config_sync() {
    let (mut state, _) = AppState::new();
    let generation = state.runtime.runtime_generation;

    // Simulate config fetch
    let _ = state.update(Message::RuntimeConfigFetched(
        Ok(RuntimeConfig {
            mode: "global".into(),
            script_block_present: true,
            tun_enabled: true,
            dns_nameservers: vec!["1.1.1.1".into()],
            dns_fallback: vec!["8.8.8.8".into()],
            dns_enhanced_mode: "fake-ip".into(),
            tun_stack: "gvisor".into(),
            tun_auto_route: true,
            tun_strict_route: false,
            sniffer_enabled: true,
        }),
        generation,
    ));

    assert_eq!(state.runtime.proxy_mode.as_ref().unwrap(), "global");
    assert!(state.runtime.tun_enabled.unwrap());
    assert_eq!(state.editor.dns_nameservers[0], "1.1.1.1");
}

#[test]
fn test_mode_set_interactions() {
    let (mut state, _) = AppState::new();

    // Success path (should trigger a re-fetch)
    let _task = state.update(Message::ModeSetResult(Ok(())));
    assert!(state.shell.error_msg.is_none());

    // Failure path
    let _ = state.update(Message::ModeSetResult(Err(InfiltratorError::Mihomo(
        "API Error".into(),
    ))));
    assert_eq!(
        state.shell.error_msg.as_ref().unwrap(),
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
    assert_eq!(state.diag.traffic.as_ref().unwrap().up, 1000);

    // No throttling currently implemented
    let _ = state.update(Message::TrafficReceived(TrafficData {
        up: 1500,
        down: 1500,
    }));
    assert_eq!(state.diag.traffic.as_ref().unwrap().up, 1500);

    // Updated
    let _ = state.update(Message::TrafficReceived(TrafficData {
        up: 3000,
        down: 3000,
    }));
    assert_eq!(state.diag.traffic.as_ref().unwrap().up, 3000);
}

#[test]
fn test_dns_server_list_manipulation() {
    let (mut state, _) = AppState::new();
    state.editor.dns_nameservers = vec!["old".into()];

    let _ = state.update(Message::UpdateDnsServer(0, "new".into()));
    assert_eq!(state.editor.dns_nameservers[0], "new");

    let _ = state.update(Message::AddDnsServer);
    assert_eq!(state.editor.dns_nameservers.len(), 2);

    let _ = state.update(Message::AddDnsServerTemplate(
        "https://1.1.1.1/dns-query".into(),
    ));
    assert_eq!(state.editor.dns_nameservers.len(), 3);
    assert_eq!(state.editor.dns_nameservers[2], "https://1.1.1.1/dns-query");

    let _ = state.update(Message::RemoveDnsServer(0));
    assert_eq!(state.editor.dns_nameservers.len(), 2);
    assert_eq!(state.editor.dns_nameservers[0], "");

    // Fallbacks
    let _ = state.update(Message::AddFallbackDnsServer);
    assert_eq!(state.editor.dns_fallback_servers.len(), 1);

    let _ = state.update(Message::UpdateFallbackDnsServer(0, "8.8.8.8".into()));
    assert_eq!(state.editor.dns_fallback_servers[0], "8.8.8.8");

    let _ = state.update(Message::RemoveFallbackDnsServer(0));
    assert_eq!(state.editor.dns_fallback_servers.len(), 0);
}

#[test]
fn test_system_integration_states() {
    let (mut state, _) = AppState::new();

    // System Proxy
    state.runtime.system_proxy_enabled = false;
    let _ = state.update(Message::SetSystemProxy(true));
    assert!(state.runtime.system_proxy_enabled);

    // Rollback on error
    let _ = state.update(Message::SystemProxySet(Err(InfiltratorError::Privilege(
        "Access denied".into(),
    ))));
    assert!(
        !state.runtime.system_proxy_enabled,
        "Should rollback on failure"
    );
    assert_eq!(
        state.shell.error_msg.as_ref().unwrap(),
        "Privilege error: Access denied"
    );

    // Autostart
    state.runtime.autostart_enabled = false;
    let _ = state.update(Message::SetAutostart(true));
    assert!(state.runtime.autostart_enabled);

    let _ = state.update(Message::AutostartSet(Err(InfiltratorError::Internal(
        "Registry lock".into(),
    ))));
    assert!(
        !state.runtime.autostart_enabled,
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
    assert_eq!(state.profile.profiles.len(), 1);
    assert!(!state.profile.is_loading_profiles);

    // Rules loaded
    let _ = state.update(Message::RulesLoaded(Ok(vec![RuleEntry {
        rule: "DOMAIN,example.com,DIRECT".into(),
        enabled: true,
    }])));
    assert_eq!(state.editor.rules.len(), 1);
}

#[test]
fn test_proxy_lifecycle_messages() {
    let (mut state, _) = AppState::new();

    let _ = state.update(Message::StartProxy);
    assert_eq!(state.runtime.status, RuntimeStatus::Starting);

    let _ = state.update(Message::ProxyStopped);
    assert!(state.diag.traffic.is_none());
}

#[test]
fn test_rebuild_flow_state_transitions() {
    let (mut state, _) = AppState::new();
    state.editor.rules = vec![RuleEntry {
        rule: "MATCH,DIRECT".into(),
        enabled: true,
    }];

    let _ = state.update(Message::SaveRules);
    assert!(matches!(
        state.runtime.rebuild_flow,
        RebuildFlowState::Saving { .. }
    ));

    let _ = state.update(Message::RuntimeRebuildFinished(Err(
        InfiltratorError::Mihomo("boom".into()),
    )));
    assert!(matches!(
        state.runtime.rebuild_flow,
        RebuildFlowState::Failed { .. }
    ));

    let _ = state.update(Message::ClearRebuildFlow);
    assert!(matches!(state.runtime.rebuild_flow, RebuildFlowState::Idle));
}

#[test]
fn test_log_buffer_limit_and_queue() {
    let (mut state, _) = AppState::new();

    for i in 0..650 {
        let _ = state.update(Message::LogReceived(format!("log {}", i)));
    }

    assert_eq!(state.diag.logs.len(), 500);
    assert_eq!(state.diag.logs.front().unwrap(), "log 150");
    assert_eq!(state.diag.logs.back().unwrap(), "log 649");
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
        state.editor.editor_path.as_ref().unwrap().to_str().unwrap(),
        "config.yaml"
    );
    assert_eq!(state.editor.editor_content.text(), "proxies: []");

    // Editor action (simulating typing)
    let _ = state.update(Message::EditorAction(text_editor::Action::Edit(
        text_editor::Edit::Insert('a'),
    )));
    assert_ne!(state.editor.editor_content.text(), "proxies: []");

    // Save success
    state.shell.current_route = Route::Editor;
    let _ = state.update(Message::ProfileSaved(Ok(())));
    assert_eq!(state.shell.current_route, Route::Editor);
}

#[test]
fn test_tray_and_exit() {
    let (mut state, _) = AppState::new();

    // Tray events shouldn't crash and must map onto the same actions as the
    // old muda menu ids: icon click shows the window, quit requests exit.
    let _ = state.update(Message::TrayEvent(
        crate::tray::spec::TrayEvent::IconActivated,
    ));
    assert_eq!(
        crate::tray::spec::resolve_tray_event(
            &crate::tray::spec::TrayEvent::MenuActivated {
                id: crate::tray::spec::TRAY_ACTION_QUIT,
                payload: None,
            },
            false,
            false,
        ),
        Some(crate::tray::spec::TrayIntent::Exit)
    );
    let _ = state.update(Message::TrayEvent(
        crate::tray::spec::TrayEvent::MenuActivated {
            id: crate::tray::spec::TRAY_ACTION_QUIT,
            payload: None,
        },
    ));

    let _ = state.update(Message::Exit);
}

#[test]
fn test_tab_state_switches() {
    let (mut state, _) = AppState::new();
    state.editor.rules_page = 3;
    let _ = state.update(Message::SetRulesTab(RulesTab::JsonEditors));
    assert_eq!(state.editor.rules_tab, RulesTab::JsonEditors);
    assert_eq!(state.editor.rules_page, 0);

    let _ = state.update(Message::SetRulesJsonTab(RulesJsonTab::Sniffer));
    assert_eq!(state.editor.rules_json_tab, RulesJsonTab::Sniffer);

    let _ = state.update(Message::SetDnsTab(DnsTab::Tun));
    assert_eq!(state.editor.dns_tab, DnsTab::Tun);
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

#[test]
fn test_error_and_toast_redaction() {
    let (mut state, _) = AppState::new();

    // set_error is the only writer of error_msg and must redact secrets.
    state.set_error("update failed: https://sub.example.com/d?token=tok1234");
    let error = state.shell.error_msg.clone().expect("error stored");
    assert!(error.contains("token=***"), "redacted error: {error}");
    assert!(!error.contains("tok1234"), "raw token leaked: {error}");

    // Every toast funnels through Message::ShowToast and is redacted there.
    let _ = state.update(Message::ShowToast(
        "secret: supersecret42".into(),
        crate::types::app::ToastStatus::Error,
    ));
    let (content, _) = state.shell.toasts[0].clone();
    assert_eq!(content, "secret: ***");
}

#[test]
fn test_p0_confirmation_is_staged_and_cancellable() {
    let (mut state, _) = AppState::new();

    let _ = state.update(Message::RequestConfirmation(ConfirmAction::DeleteProfile(
        "unused".to_string(),
    )));
    assert_eq!(
        state.shell.confirmation,
        Some(ConfirmAction::DeleteProfile("unused".to_string()))
    );
    let _ = state.update(Message::CancelConfirmation);
    assert!(state.shell.confirmation.is_none());

    let _ = state.update(Message::RequestConfirmation(
        ConfirmAction::CloseAllConnections,
    ));
    let _ = state.update(Message::ConfirmAction);
    assert!(state.shell.confirmation.is_none());
    assert!(state.shell.error_msg.is_none());
}

#[test]
fn test_p0_runtime_stream_generation_and_failure_projection() {
    let (mut state, _) = AppState::new();
    state.runtime.runtime_generation = 7;

    let _ = state.update(Message::RuntimeStreamLogReceived(6, "stale".to_string()));
    assert!(state.diag.logs.is_empty());
    let _ = state.update(Message::RuntimeStreamLogReceived(7, "current".to_string()));
    assert_eq!(state.diag.logs.back().map(String::as_str), Some("current"));

    let _ = state.update(Message::RuntimeStreamStateChanged {
        kind: RuntimeStreamKind::Traffic,
        generation: 7,
        state: RuntimeStreamState::Failed("socket closed".to_string()),
    });
    assert!(matches!(
        state.diag.traffic_stream_state,
        RuntimeStreamState::Failed(_)
    ));
    assert!(state.shell.error_msg.is_some());
}

#[test]
fn test_p0_ip_probe_metadata_and_explicit_language() {
    let (mut state, _) = AppState::new();
    let task_id = state.shell.last_task_id;
    let _ = state.update(Message::IpInfoReceived(
        Ok(IpProbeResult {
            ip: "203.0.113.10".to_string(),
            provider: "test-provider".to_string(),
            checked_at: "2026-08-30 12:00:00".to_string(),
        }),
        task_id,
    ));
    assert_eq!(state.diag.public_ip.as_deref(), Some("203.0.113.10"));
    assert_eq!(
        state.diag.public_ip_provider.as_deref(),
        Some("test-provider")
    );
    assert_eq!(
        state.diag.public_ip_checked_at.as_deref(),
        Some("2026-08-30 12:00:00")
    );

    let _ = state.update(Message::SetLanguage("en-US".to_string()));
    assert_eq!(state.shell.lang, "en-US");
}

#[test]
fn test_p0_download_progress_token_and_sync_progress() {
    let (mut state, _) = AppState::new();
    state.runtime.core_download_token = 9;
    state.runtime.is_downloading_core = true;
    let stale = CoreDownloadProgress {
        downloaded: 100,
        total: Some(200),
        speed_bytes: 10,
    };
    let _ = state.update(Message::CoreDownloadProgress(stale, 8));
    assert!(state.runtime.download_stats.is_none());

    let current = CoreDownloadProgress {
        downloaded: 100,
        total: Some(200),
        speed_bytes: 10,
    };
    let _ = state.update(Message::CoreDownloadProgress(current, 9));
    assert_eq!(state.runtime.download_progress, 0.5);
    assert!(state.runtime.download_stats.is_some());

    let _ = state.update(Message::SyncProgress(SyncProgress {
        phase: "上传配置".to_string(),
        current: 2,
        total: 4,
    }));
    assert_eq!(
        state.profile.sync_progress.as_ref().map(|p| p.current),
        Some(2)
    );
}

#[test]
fn test_p0_stale_proxy_start_is_ignored() {
    let (mut state, _) = AppState::new();
    state.runtime.lifecycle_token = 4;
    let _ = state.update(Message::ProxyStarted(
        Err(InfiltratorError::Mihomo("late start".to_string())),
        3,
    ));
    assert_eq!(state.runtime.status, RuntimeStatus::Stopped);
    assert!(state.shell.error_msg.is_none());
}

#[test]
fn test_p0_runtime_patch_failure_restores_the_previous_snapshot() {
    let (mut state, _) = AppState::new();
    state.runtime.proxy_mode = Some("rule".to_string());
    state.runtime.pending_runtime_patch = Some(RuntimePatchSnapshot {
        proxy_mode: Some("rule".to_string()),
        tun_enabled: Some(false),
        tun_stack: "gvisor".to_string(),
        tun_auto_route: true,
        tun_strict_route: false,
        sniffer_enabled: true,
    });
    state.runtime.runtime_patch_token = 11;
    state.runtime.proxy_mode = Some("global".to_string());
    let generation = state.runtime.runtime_generation;

    let _ = state.update(Message::RuntimePatchResult(
        Err(InfiltratorError::Mihomo("controller rejected".to_string())),
        11,
        generation,
    ));
    assert_eq!(state.runtime.proxy_mode.as_deref(), Some("rule"));
    assert!(state.runtime.pending_runtime_patch.is_none());
    assert!(state.shell.error_msg.is_some());

    state.runtime.proxy_mode = Some("direct".to_string());
    let generation = state.runtime.runtime_generation;
    let _ = state.update(Message::RuntimePatchResult(Ok(()), 10, generation));
    assert_eq!(state.runtime.proxy_mode.as_deref(), Some("direct"));
}

#[test]
fn connections_pagination_windows_and_clamps() {
    use mihomo_api::types::{Connection, ConnectionMetadata, ConnectionSnapshot};

    let snapshot_with = |count: usize| ConnectionSnapshot {
        download_total: 0,
        upload_total: 0,
        connections: (0..count)
            .map(|i| Connection {
                id: i.to_string(),
                metadata: ConnectionMetadata::default(),
                upload: 0,
                download: 0,
                start: String::new(),
                rule: String::new(),
                rule_payload: String::new(),
                chains: Vec::new(),
            })
            .collect(),
    };

    let (mut state, _) = AppState::new();
    state.diag.connections_page_size = 100;

    // 250 connections → 3 pages; next from page 0 → 1 → 2, then clamps at 2.
    let _ = state.update(Message::ConnectionsReceived(snapshot_with(250)));
    for expected in [1usize, 2, 2] {
        let _ = state.update(Message::ConnectionsNextPage);
        assert_eq!(state.diag.connections_page, expected);
    }
    let (page, start, end) = state.connections_window(250);
    assert_eq!((page, start, end), (2, 200, 250));

    // Snapshot shrinks below the current page → clamped back into range.
    let _ = state.update(Message::ConnectionsReceived(snapshot_with(80)));
    let (page, start, end) = state.connections_window(80);
    assert_eq!((page, start, end), (0, 0, 80));
    assert_eq!(state.diag.connections_page, 0);

    // Prev from page 0 saturates at 0.
    let _ = state.update(Message::ConnectionsPrevPage);
    assert_eq!(state.diag.connections_page, 0);

    // Filter/sort changes reset to the first page.
    let _ = state.update(Message::ConnectionsReceived(snapshot_with(250)));
    let _ = state.update(Message::ConnectionsNextPage);
    let _ = state.update(Message::UpdateRuntimeConnectionFilter("x".into()));
    assert_eq!(state.diag.connections_page, 0);
    let _ = state.update(Message::ConnectionsNextPage);
    let _ = state.update(Message::UpdateRuntimeConnectionSort("host_asc".into()));
    assert_eq!(state.diag.connections_page, 0);
}
