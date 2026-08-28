use super::*;
use crate::locales::{Lang, Localizer};
use crate::types::{
    AdvancedConfigsBundle, AdvancedEditMode, DnsTab, EditorLazyState, RebuildFlowState,
    RulesJsonTab, RulesTab, RuntimeConfig,
};
use iced::widget::text_editor;
use infiltrator_core::rules::RuleEntry;
use infiltrator_core::settings::{AppSettings, RuntimePanelConfig};
use mihomo_api::{Proxy, ProxyBase, ProxyGroup, ProxyHistory, TrafficData};
use mihomo_config::Profile;
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

    // Tray event shouldn't crash
    let _ = state.update(Message::TrayIconEvent(tray_icon::TrayIconEvent::Click {
        id: "test".into(),
        position: tray_icon::dpi::PhysicalPosition { x: 0.0, y: 0.0 },
        rect: tray_icon::Rect {
            position: tray_icon::dpi::PhysicalPosition { x: 0.0, y: 0.0 },
            size: tray_icon::dpi::PhysicalSize {
                width: 0,
                height: 0,
            },
        },
        button: tray_icon::MouseButton::Left,
        button_state: tray_icon::MouseButtonState::Up,
    }));

    let _ = state.update(Message::Exit);
}

#[test]
fn test_proxy_filtering_and_sorting() {
    let (mut state, _) = AppState::new();
    let mut proxies = std::collections::HashMap::new();

    // Group GLOBAL
    proxies.insert(
        "GLOBAL".to_string(),
        Proxy::Selector(ProxyGroup {
            name: "GLOBAL".to_string(),
            now: "Proxy-A".to_string(),
            all: vec!["Proxy-A".into(), "Proxy-B".into(), "Special".into()],
            history: vec![],
        }),
    );

    // Node A (100ms)
    proxies.insert(
        "Proxy-A".to_string(),
        Proxy::Shadowsocks(mihomo_api::proxy::types::Shadowsocks {
            base: ProxyBase {
                name: "Proxy-A".to_string(),
                history: vec![ProxyHistory {
                    time: "".into(),
                    delay: 100,
                }],
                ..Default::default()
            },
            ..Default::default()
        }),
    );

    // Node B (50ms)
    proxies.insert(
        "Proxy-B".to_string(),
        Proxy::Shadowsocks(mihomo_api::proxy::types::Shadowsocks {
            base: ProxyBase {
                name: "Proxy-B".to_string(),
                history: vec![ProxyHistory {
                    time: "".into(),
                    delay: 50,
                }],
                ..Default::default()
            },
            ..Default::default()
        }),
    );

    // Node Special (200ms)
    proxies.insert(
        "Special".to_string(),
        Proxy::Shadowsocks(mihomo_api::proxy::types::Shadowsocks {
            base: ProxyBase {
                name: "Special".to_string(),
                history: vec![ProxyHistory {
                    time: "".into(),
                    delay: 200,
                }],
                ..Default::default()
            },
            ..Default::default()
        }),
    );

    state.proxies = proxies;

    // Test Search
    let _ = state.update(Message::FilterProxies("special".into()));
    assert_eq!(state.proxy_filter, "special");

    // Test Sort Toggle
    let _ = state.update(Message::ToggleProxySort);
    assert!(state.proxy_sort_by_delay);

    // Verification of logic (manual check of sorting logic used in view)
    let global = state.proxies.get("GLOBAL").unwrap();
    let mut members = global.all().unwrap().to_vec();

    // Apply filter
    let filter = state.proxy_filter.to_lowercase();
    members.retain(|m| m.to_lowercase().contains(&filter));
    assert_eq!(members.len(), 1);
    assert_eq!(members[0], "Special");

    // Apply sort (on all members)
    let mut all_members = global.all().unwrap().to_vec();
    all_members.sort_by_key(|m| {
        state
            .proxies
            .get(m)
            .and_then(|p| p.history().last().map(|h| h.delay))
            .unwrap_or(u32::MAX)
    });

    assert_eq!(all_members[0], "Proxy-B"); // 50ms
    assert_eq!(all_members[1], "Proxy-A"); // 100ms
    assert_eq!(all_members[2], "Special"); // 200ms
}

#[test]
fn test_runtime_auto_refresh_toggle() {
    let (mut state, _) = AppState::new();
    assert!(state.runtime_auto_refresh);

    let _ = state.update(Message::UpdateRuntimeAutoRefresh(false));
    assert!(!state.runtime_auto_refresh);

    let _ = state.update(Message::UpdateRuntimeAutoRefresh(true));
    assert!(state.runtime_auto_refresh);
}

#[test]
fn test_runtime_connection_sort_mode_switch() {
    let (mut state, _) = AppState::new();
    assert_eq!(state.runtime_connection_sort, "download_desc");

    let _ = state.update(Message::UpdateRuntimeConnectionSort("upload_desc".into()));
    assert_eq!(state.runtime_connection_sort, "upload_desc");

    let _ = state.update(Message::UpdateRuntimeConnectionSort("invalid_key".into()));
    assert_eq!(state.runtime_connection_sort, "download_desc");
}

#[test]
fn test_proxy_delay_sort_mode_switch() {
    let (mut state, _) = AppState::new();

    let _ = state.update(Message::UpdateProxyDelaySort("name_desc".into()));
    assert_eq!(state.proxy_delay_sort, "name_desc");
    assert!(!state.proxy_sort_by_delay);

    let _ = state.update(Message::UpdateProxyDelaySort("delay_desc".into()));
    assert_eq!(state.proxy_delay_sort, "delay_desc");
    assert!(state.proxy_sort_by_delay);
}

#[test]
fn test_profiles_filter_state() {
    let (mut state, _) = AppState::new();
    let _ = state.update(Message::UpdateProfilesFilter("default".into()));
    assert_eq!(state.profiles_filter, "default");
}

#[test]
fn test_runtime_connection_filter_state() {
    let (mut state, _) = AppState::new();
    let _ = state.update(Message::UpdateRuntimeConnectionFilter("api".into()));
    assert_eq!(state.runtime_connection_filter, "api");
}

#[test]
fn test_runtime_proxy_selector_sync_and_apply() {
    let (mut state, _) = AppState::new();
    let mut proxies = std::collections::HashMap::new();

    proxies.insert(
        "GLOBAL".to_string(),
        Proxy::Selector(ProxyGroup {
            name: "GLOBAL".to_string(),
            now: "Proxy-A".to_string(),
            all: vec!["Proxy-A".into(), "Proxy-B".into()],
            history: vec![],
        }),
    );
    proxies.insert(
        "Proxy-A".to_string(),
        Proxy::Shadowsocks(mihomo_api::proxy::types::Shadowsocks {
            base: ProxyBase {
                name: "Proxy-A".to_string(),
                ..Default::default()
            },
            ..Default::default()
        }),
    );
    proxies.insert(
        "Proxy-B".to_string(),
        Proxy::Shadowsocks(mihomo_api::proxy::types::Shadowsocks {
            base: ProxyBase {
                name: "Proxy-B".to_string(),
                ..Default::default()
            },
            ..Default::default()
        }),
    );

    let _ = state.update(Message::ProxiesLoaded(Ok(proxies)));
    assert_eq!(state.runtime_selected_group, "GLOBAL");
    assert_eq!(state.runtime_selected_proxy, "Proxy-A");

    let _ = state.update(Message::UpdateRuntimeSelectedProxy("Proxy-B".into()));
    assert_eq!(state.runtime_selected_proxy, "Proxy-B");

    let _ = state.update(Message::ApplyRuntimeSelectedProxy);
}

#[test]
fn test_settings_loaded_applies_runtime_panel_state() {
    let (mut state, _) = AppState::new();
    let settings = AppSettings {
        runtime_panel: RuntimePanelConfig {
            auto_refresh: false,
            delay_sort: "name_desc".into(),
            delay_test_url: "https://example.com/generate_204".into(),
            delay_timeout_ms: 1200,
            connection_filter: "api".into(),
            connection_sort: "host_asc".into(),
        },
        ..AppSettings::default()
    };

    let _ = state.update(Message::SettingsLoaded(Ok(settings)));
    assert!(!state.runtime_auto_refresh);
    assert_eq!(state.proxy_delay_sort, "name_desc");
    assert_eq!(
        state.runtime_delay_test_url,
        "https://example.com/generate_204"
    );
    assert_eq!(state.runtime_delay_timeout_ms, "1200");
    assert_eq!(state.runtime_connection_filter, "api");
    assert_eq!(state.runtime_connection_sort, "host_asc");
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
fn test_rules_render_cache_and_filter() {
    let (mut state, _) = AppState::new();
    let _ = state.update(Message::RulesLoaded(Ok(vec![
        RuleEntry {
            rule: "DOMAIN,example.com,DIRECT".into(),
            enabled: true,
        },
        RuleEntry {
            rule: "IP-CIDR,10.0.0.0/8,REJECT".into(),
            enabled: true,
        },
        RuleEntry {
            rule: "DOMAIN-SUFFIX,example.net,GLOBAL".into(),
            enabled: true,
        },
    ])));

    assert_eq!(state.rules_render_cache.len(), 3);
    assert_eq!(state.rules_filtered_indices.len(), 3);
    assert_eq!(state.rules_render_cache[0].payload, "example.com");

    let _ = state.update(Message::FilterRules("example.net".into()));
    assert_eq!(state.rules_page, 0);
    assert_eq!(state.rules_filtered_indices.len(), 1);
}

#[test]
fn test_rules_pagination_bounds() {
    let (mut state, _) = AppState::new();
    let rules: Vec<RuleEntry> = (0..450)
        .map(|i| RuleEntry {
            rule: format!("DOMAIN,host-{i}.example,DIRECT"),
            enabled: true,
        })
        .collect();
    let _ = state.update(Message::RulesLoaded(Ok(rules)));
    assert_eq!(state.rules_page_size, 200);

    let _ = state.update(Message::RulesNextPage);
    let _ = state.update(Message::RulesNextPage);
    let _ = state.update(Message::RulesNextPage); // should clamp to last
    assert_eq!(state.rules_page, 2);

    let _ = state.update(Message::RulesPrevPage);
    assert_eq!(state.rules_page, 1);

    let _ = state.update(Message::RulesSetPage(99));
    assert_eq!(state.rules_page, 2);
}

#[test]
fn test_rules_dns_lazy_editor_state() {
    let (mut state, _) = AppState::new();

    assert_eq!(state.rule_providers_editor_state, EditorLazyState::Unloaded);
    state.rule_providers_json_cache = "{\"a\":1}".into();
    let _ = state.update(Message::EnsureRuleProvidersEditorLoaded);
    assert_eq!(state.rule_providers_editor_state, EditorLazyState::Loaded);
    assert_eq!(state.rule_providers_json_content.text(), "{\"a\":1}");

    assert_eq!(state.dns_editor_state, EditorLazyState::Unloaded);
    state.dns_json_cache = "{\"enable\":true}".into();
    let _ = state.update(Message::EnsureDnsEditorLoaded);
    assert_eq!(state.dns_editor_state, EditorLazyState::Loaded);
    assert_eq!(state.dns_json_content.text(), "{\"enable\":true}");
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
fn test_rules_dns_large_sample_smoke() {
    let (mut state, _) = AppState::new();
    let rules: Vec<RuleEntry> = (0..3200)
        .map(|i| RuleEntry {
            rule: format!("DOMAIN,host-{i}.example,DIRECT"),
            enabled: true,
        })
        .collect();
    let _ = state.update(Message::RulesLoaded(Ok(rules)));
    assert_eq!(state.rules_render_cache.len(), 3200);

    let large_json = "a".repeat(1024 * 1024);
    state.dns_json_cache = format!("{{\"dns\":\"{}\"}}", large_json);
    state.fake_ip_json_cache = format!("{{\"fake\":\"{}\"}}", large_json);
    state.tun_json_cache = format!("{{\"tun\":\"{}\"}}", large_json);
    let _ = state.update(Message::EnsureDnsEditorLoaded);
    let _ = state.update(Message::EnsureFakeIpEditorLoaded);
    let _ = state.update(Message::EnsureTunEditorLoaded);
    assert_eq!(state.dns_editor_state, EditorLazyState::Loaded);
    assert_eq!(state.fake_ip_editor_state, EditorLazyState::Loaded);
    assert_eq!(state.tun_editor_state, EditorLazyState::Loaded);
}

#[test]
fn test_dns_form_dirty_and_json_sync() {
    let (mut state, _) = AppState::new();
    let _ = state.update(Message::UpdateDnsFormNameserver(
        "1.1.1.1, 8.8.8.8".to_string(),
    ));
    assert!(state.dns_form_dirty);
    let patch: infiltrator_core::dns::DnsConfigPatch =
        serde_json::from_str(&state.dns_json_cache).expect("dns patch json");
    assert_eq!(
        patch.nameserver,
        Some(vec!["1.1.1.1".to_string(), "8.8.8.8".to_string()])
    );
}

#[test]
fn test_set_advanced_mode_updates_state() {
    let (mut state, _) = AppState::new();
    let _ = state.update(Message::SetAdvancedMode(
        DnsTab::Dns,
        AdvancedEditMode::Json,
    ));
    assert_eq!(state.dns_mode, AdvancedEditMode::Json);
}

#[test]
fn test_tun_form_invalid_mtu_blocks_save() {
    let (mut state, _) = AppState::new();
    let _ = state.update(Message::UpdateTunFormMtu("abc".to_string()));
    let _ = state.update(Message::SaveTunConfig);
    assert!(!state.is_saving_tun);
    assert!(matches!(
        state.rebuild_flow,
        RebuildFlowState::Failed { .. }
    ));
    assert!(
        state
            .advanced_validation
            .tun
            .as_ref()
            .is_some_and(|msg| msg.to_ascii_lowercase().contains("mtu"))
    );
}

#[test]
fn test_advanced_bundle_load_applies_form_drafts() {
    let (mut state, _) = AppState::new();
    let bundle = AdvancedConfigsBundle {
        dns_json: "{}".to_string(),
        fake_ip_json: "{}".to_string(),
        tun_json: "{}".to_string(),
        dns: infiltrator_core::dns::DnsConfig {
            enable: Some(true),
            nameserver: Some(vec!["https://dns.google/dns-query".to_string()]),
            enhanced_mode: Some("fake-ip".to_string()),
            ..Default::default()
        },
        fake_ip: infiltrator_core::fake_ip::FakeIpConfig {
            fake_ip_range: Some("198.18.0.1/16".to_string()),
            store_fake_ip: Some(true),
            ..Default::default()
        },
        tun: infiltrator_core::tun::TunConfig {
            enable: Some(true),
            stack: Some("gvisor".to_string()),
            mtu: Some(1500),
            ..Default::default()
        },
    };
    let _ = state.update(Message::AdvancedConfigsBundleLoaded(Ok(bundle)));
    assert!(state.dns_form.enable);
    assert_eq!(
        state.dns_form.nameserver,
        "https://dns.google/dns-query".to_string()
    );
    assert_eq!(
        state.fake_ip_form.fake_ip_range,
        "198.18.0.1/16".to_string()
    );
    assert!(state.fake_ip_form.store_fake_ip);
    assert!(state.tun_form.enable);
    assert_eq!(state.tun_form.stack, "gvisor".to_string());
    assert_eq!(state.tun_form.mtu, "1500".to_string());
}
