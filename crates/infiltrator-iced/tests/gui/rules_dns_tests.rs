//! Rules & DNS/advanced-config tests: render cache + filtering, pagination
//! bounds, lazy JSON editors, form drafts, validation and heavy-sample smoke.
//! Mounted via `src/test_mounts.rs` (crate root).
//! test-intent: behavior

use crate::types::dns::{AdvancedConfigsBundle, AdvancedEditMode, DnsTab};
use crate::types::editor::EditorLazyState;
use crate::types::runtime::RebuildFlowState;
use crate::state::AppState;
use crate::types::message::Message;
use infiltrator_core::rules::RuleEntry;

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

    assert_eq!(state.editor.rules_render_cache.len(), 3);
    assert_eq!(state.editor.rules_filtered_indices.len(), 3);
    assert_eq!(state.editor.rules_render_cache[0].payload, "example.com");

    let _ = state.update(Message::FilterRules("example.net".into()));
    assert_eq!(state.editor.rules_page, 0);
    assert_eq!(state.editor.rules_filtered_indices.len(), 1);
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
    assert_eq!(state.editor.rules_page_size, 200);

    let _ = state.update(Message::RulesNextPage);
    let _ = state.update(Message::RulesNextPage);
    let _ = state.update(Message::RulesNextPage); // should clamp to last
    assert_eq!(state.editor.rules_page, 2);

    let _ = state.update(Message::RulesPrevPage);
    assert_eq!(state.editor.rules_page, 1);

    let _ = state.update(Message::RulesSetPage(99));
    assert_eq!(state.editor.rules_page, 2);
}

#[test]
fn test_rules_dns_lazy_editor_state() {
    let (mut state, _) = AppState::new();

    assert_eq!(
        state.editor.rule_providers_editor_state,
        EditorLazyState::Unloaded
    );
    state.editor.rule_providers_json_cache = "{\"a\":1}".into();
    let _ = state.update(Message::EnsureRuleProvidersEditorLoaded);
    assert_eq!(
        state.editor.rule_providers_editor_state,
        EditorLazyState::Loaded
    );
    assert_eq!(state.editor.rule_providers_json_content.text(), "{\"a\":1}");

    assert_eq!(state.editor.dns_editor_state, EditorLazyState::Unloaded);
    state.editor.dns_json_cache = "{\"enable\":true}".into();
    let _ = state.update(Message::EnsureDnsEditorLoaded);
    assert_eq!(state.editor.dns_editor_state, EditorLazyState::Loaded);
    assert_eq!(state.editor.dns_json_content.text(), "{\"enable\":true}");
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
    assert_eq!(state.editor.rules_render_cache.len(), 3200);

    let large_json = "a".repeat(1024 * 1024);
    state.editor.dns_json_cache = format!("{{\"dns\":\"{}\"}}", large_json);
    state.editor.fake_ip_json_cache = format!("{{\"fake\":\"{}\"}}", large_json);
    state.editor.tun_json_cache = format!("{{\"tun\":\"{}\"}}", large_json);
    let _ = state.update(Message::EnsureDnsEditorLoaded);
    let _ = state.update(Message::EnsureFakeIpEditorLoaded);
    let _ = state.update(Message::EnsureTunEditorLoaded);
    assert_eq!(state.editor.dns_editor_state, EditorLazyState::Loaded);
    assert_eq!(state.editor.fake_ip_editor_state, EditorLazyState::Loaded);
    assert_eq!(state.editor.tun_editor_state, EditorLazyState::Loaded);
}

#[test]
fn test_dns_form_dirty_and_json_sync() {
    let (mut state, _) = AppState::new();
    let _ = state.update(Message::UpdateDnsFormNameserver(
        "1.1.1.1, 8.8.8.8".to_string(),
    ));
    assert!(state.editor.dns_form_dirty);
    let patch: infiltrator_core::dns::DnsConfigPatch =
        serde_json::from_str(&state.editor.dns_json_cache).expect("dns patch json");
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
    assert_eq!(state.editor.dns_mode, AdvancedEditMode::Json);
}

#[test]
fn test_tun_form_invalid_mtu_blocks_save() {
    let (mut state, _) = AppState::new();
    let _ = state.update(Message::UpdateTunFormMtu("abc".to_string()));
    let _ = state.update(Message::SaveTunConfig);
    assert!(!state.editor.is_saving_tun);
    assert!(matches!(
        state.runtime.rebuild_flow,
        RebuildFlowState::Failed { .. }
    ));
    assert!(
        state
            .editor
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
    let _ = state.update(Message::AdvancedConfigsBundleLoaded(Ok(Box::new(bundle))));
    assert!(state.editor.dns_form.enable);
    assert_eq!(
        state.editor.dns_form.nameserver,
        "https://dns.google/dns-query".to_string()
    );
    assert_eq!(
        state.editor.fake_ip_form.fake_ip_range,
        "198.18.0.1/16".to_string()
    );
    assert!(state.editor.fake_ip_form.store_fake_ip);
    assert!(state.editor.tun_form.enable);
    assert_eq!(state.editor.tun_form.stack, "gvisor".to_string());
    assert_eq!(state.editor.tun_form.mtu, "1500".to_string());
}
