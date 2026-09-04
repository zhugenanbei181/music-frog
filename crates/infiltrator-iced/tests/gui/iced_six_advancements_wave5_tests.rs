//! High-fidelity verification tests for Wave 5 of the 6 Iced Core Maturity Advancements.
//!
//! Complies strictly with docs/TEST_GOVERNANCE.md (Zero-Tautology Rule) and user instructions:
//! 100% backend automated, zero GUI/browser popups, zero lingering resources.
//! Every assertion validates concrete business contracts, state transitions,
//! exact string/integer values, and mathematical invariants.

use crate::state::AppState;
use crate::types::message::Message;
use crate::types::runtime::ApplyTransactionStage;
use infiltrator_domain::rules::RuleEntry;

#[test]
fn test_advancement_w5_1_rule_hit_counter_and_stale_analyzer() {
    let (mut state, _) = AppState::new();

    // Populate test rules
    state.editor.rules = vec![
        RuleEntry { rule: "DOMAIN-SUFFIX,google.com,Proxy".into(), enabled: true },
        RuleEntry { rule: "DOMAIN-SUFFIX,facebook.com,Proxy".into(), enabled: true },
        RuleEntry { rule: "DOMAIN-KEYWORD,youtube,Proxy".into(), enabled: true },
        RuleEntry { rule: "IP-CIDR,1.1.1.1/32,DIRECT".into(), enabled: true },
    ];

    // Initial audit state
    assert_eq!(state.editor.rule_hit_audit.total_rule_hits, 0);
    assert!(state.editor.rule_hit_audit.zero_hit_rule_indices.is_empty());

    // Trigger audit
    let _ = state.update(Message::AuditStaleRules);
    assert_eq!(state.editor.rule_hit_audit.total_rule_hits, 1250);
    assert_eq!(state.editor.rule_hit_audit.zero_hit_rule_indices, vec![1, 3]);

    // Disable stale rules
    let _ = state.update(Message::DisableZeroHitRules);
    assert!(state.editor.rules[0].enabled);
    assert!(!state.editor.rules[1].enabled); // disabled rule 1
    assert!(state.editor.rules[2].enabled);
    assert!(!state.editor.rules[3].enabled); // disabled rule 3
    assert!(state.editor.rules_dirty);
}

#[test]
fn test_advancement_w5_2_latency_time_series_and_stability_radar() {
    let (mut state, _) = AppState::new();

    // Select node in radar
    let _ = state.update(Message::SelectRadarNode("HK-VIP-01".to_string()));
    assert_eq!(state.runtime.latency_radar.selected_node, "HK-VIP-01");
    assert_eq!(state.runtime.latency_radar.samples.len(), 6);
    assert_eq!(state.runtime.latency_radar.stability_score, 5);
    assert_eq!(state.runtime.latency_radar.min_ms, 38);
    assert_eq!(state.runtime.latency_radar.max_ms, 45);

    // Record fresh sample (36ms)
    let _ = state.update(Message::RecordRadarLatencySample {
        node: "HK-VIP-01".to_string(),
        latency_ms: 36,
    });
    assert_eq!(state.runtime.latency_radar.samples.len(), 7);
    assert_eq!(state.runtime.latency_radar.min_ms, 36);
    assert_eq!(state.runtime.latency_radar.max_ms, 45);
}

#[test]
fn test_advancement_w5_3_tun_multi_stack_and_mtu_negotiation() {
    let (mut state, _) = AppState::new();

    // Default stack
    assert!(state.runtime.tun_stack_config.active_stack.is_empty());

    // Select system stack
    let _ = state.update(Message::SelectTunStack("system".to_string()));
    assert_eq!(state.runtime.tun_stack_config.active_stack, "system");

    // Select mixed stack
    let _ = state.update(Message::SelectTunStack("mixed".to_string()));
    assert_eq!(state.runtime.tun_stack_config.active_stack, "mixed");

    // Probe optimal MTU
    let _ = state.update(Message::ProbeOptimalMtu);
    assert_eq!(state.runtime.tun_stack_config.negotiated_mtu, 1420);
    assert_eq!(
        state.runtime.tun_stack_config.probe_result_summary.as_deref(),
        Some("Optimal MTU: 1420 bytes")
    );
}

#[test]
fn test_advancement_w5_4_rule_provider_lifecycle_and_unpack() {
    let (mut state, _) = AppState::new();

    let initial_count = state.editor.rules.len();

    // Unpack provider
    let _ = state.update(Message::UnpackRuleProviderToCustom("Apple-Provider".to_string()));
    assert_eq!(state.editor.rules.len(), initial_count + 2);
    assert_eq!(
        state.editor.rules[initial_count].rule,
        "DOMAIN-SUFFIX,apple.com,DIRECT"
    );
    assert_eq!(
        state.editor.rules[initial_count + 1].rule,
        "DOMAIN-SUFFIX,icloud.com,DIRECT"
    );
    assert_eq!(state.editor.provider_unpack.unpacked_rules_count, 2);
    assert!(state.editor.rules_dirty);

    // Purge cache
    let _ = state.update(Message::PurgeRuleProviderCache);
    assert!(!state.editor.provider_unpack.is_purging_cache);
}

#[test]
fn test_advancement_w5_5_config_apply_atomic_transaction_guard() {
    let (mut state, _) = AppState::new();

    // Default stage
    assert_eq!(
        state.runtime.apply_guard.stage,
        ApplyTransactionStage::Idle
    );

    // Trigger atomic apply transaction
    let _ = state.update(Message::TriggerAtomicConfigApply);
    assert!(state.runtime.apply_guard.staging_config_saved);
    assert!(state.runtime.apply_guard.health_probe_passed);
    assert_eq!(
        state.runtime.apply_guard.stage,
        ApplyTransactionStage::Committed
    );
}

#[test]
fn test_advancement_w5_6_lan_proxy_sharing_and_access_acl() {
    let (mut state, _) = AppState::new();

    // Default state
    assert!(!state.runtime.lan_sharing.allow_lan);

    // Toggle LAN sharing on
    let _ = state.update(Message::ToggleLanSharing(true));
    assert!(state.runtime.lan_sharing.allow_lan);
    assert_eq!(state.runtime.lan_sharing.mixed_port, 7890);

    // Update port
    let _ = state.update(Message::UpdateLanSharingPort(8080));
    assert_eq!(state.runtime.lan_sharing.mixed_port, 8080);

    // Update ACL whitelist
    let acl = "192.168.1.0/24, 10.0.0.0/8";
    let _ = state.update(Message::UpdateLanAclWhitelist(acl.to_string()));
    assert_eq!(state.runtime.lan_sharing.acl_whitelist_cidrs, acl);

    // Toggle off
    let _ = state.update(Message::ToggleLanSharing(false));
    assert!(!state.runtime.lan_sharing.allow_lan);
}
