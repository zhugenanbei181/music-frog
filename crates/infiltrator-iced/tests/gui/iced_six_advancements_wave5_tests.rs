//! High-fidelity verification tests for Wave 5 of the 6 Iced Core Maturity Advancements.
//!
//! Complies strictly with docs/TEST_GOVERNANCE.md (Zero-Tautology Rule) and user instructions:
//! 100% backend automated, zero GUI/browser popups, zero lingering resources.
//! Every assertion validates concrete business contracts, state transitions,
//! exact string/integer values, and mathematical invariants.

use crate::state::AppState;
use crate::types::message::{Message, MtuProbeCompletion};
use crate::types::runtime::ApplyTransactionStage;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::ipv6::Ipv6RoutingSnapshot;
use infiltrator_contract::lan::{LanSecuritySnapshot, LanSharingSnapshot};
use infiltrator_contract::mtu::{MtuNegotiationSnapshot, PhysicalMtuSnapshot};
use infiltrator_contract::rule_tracer::{
    RuleDeadEntry, RuleDeadReason, RuleHitAuditSnapshot, RuleTracerSnapshot,
};
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::{PageData, SettingsPageSnapshot, SurfaceSnapshot};
use infiltrator_contract::system_proxy::{
    SystemProxyObservation, SystemProxyRecoverySnapshot, SystemProxyRecoveryStatus,
    SystemProxyStatus,
};
use infiltrator_contract::tun::{TunStack, TunStackAvailability};
use infiltrator_domain::rules::RuleEntry;

fn rules_page_with_hit_audit(audit: RuleHitAuditSnapshot) -> SurfaceSnapshot {
    let mut snapshot = SurfaceSnapshot::unavailable(
        SurfaceKind::IcedDesktop,
        HostKind::Desktop,
        Failure::new(ErrorCode::NotReady, "test snapshot", true),
    );
    snapshot.revision = 3;
    snapshot.pages.rules =
        PageData::ready(infiltrator_contract::surface_snapshot::RulesPageSnapshot {
            total_rules: 4,
            default_action: "Proxy".to_owned(),
            providers: Vec::new(),
            rules: Vec::new(),
            tracer: RuleTracerSnapshot {
                hit_audit: audit,
                ..Default::default()
            },
            mrs_acceleration: Default::default(),
            total_hits: 0,
            rule_publish_limit: 0,
        });
    snapshot
}

#[test]
fn test_advancement_w5_1_rule_hit_counter_and_stale_analyzer() {
    let (mut state, _) = AppState::new();

    // Populate test rules
    state.editor.rules = vec![
        RuleEntry {
            rule: "DOMAIN-SUFFIX,google.com,Proxy".into(),
            enabled: true,
        },
        RuleEntry {
            rule: "DOMAIN-SUFFIX,facebook.com,Proxy".into(),
            enabled: true,
        },
        RuleEntry {
            rule: "DOMAIN-KEYWORD,youtube,Proxy".into(),
            enabled: true,
        },
        RuleEntry {
            rule: "IP-CIDR,1.1.1.1/32,DIRECT".into(),
            enabled: true,
        },
    ];

    // No fabricated audit before the shared projection arrives.
    assert_eq!(state.editor.rule_hit_audit.audit.total_hits, 0);
    assert!(state.editor.rule_hit_audit.zero_hit_rule_indices.is_empty());

    // The application-owned hit audit reaches the Iced projection verbatim.
    let audit = RuleHitAuditSnapshot {
        total_hits: 42,
        tracked_rules: 2,
        top_hits: Vec::new(),
        dead_rules: vec![
            RuleDeadEntry {
                rule_raw: "DOMAIN-SUFFIX,facebook.com,Proxy".into(),
                hit_count: 0,
                reason: RuleDeadReason::ZeroHits,
                shadowed_by: None,
                detail: None,
                last_hit_secs: None,
            },
            RuleDeadEntry {
                rule_raw: "IP-CIDR,1.1.1.1/32,DIRECT".into(),
                hit_count: 0,
                reason: RuleDeadReason::Shadowed,
                shadowed_by: Some("IP-CIDR,0.0.0.0/0,DIRECT".into()),
                detail: Some("shadowed".into()),
                last_hit_secs: None,
            },
        ],
        cidr_overlaps: Vec::new(),
        last_hit_rule: Some("DOMAIN-SUFFIX,google.com,Proxy".into()),
        last_hit_secs: Some(1_700_000_000),
        can_clear: true,
        trace_count: 3,
        avg_match_latency_us: Some(12.5),
        last_match_latency_us: Some(11),
    };
    assert!(state.apply_shared_surface_snapshot(rules_page_with_hit_audit(audit)));
    assert_eq!(state.editor.rule_hit_audit.audit.total_hits, 42);
    assert_eq!(
        state.editor.rule_hit_audit.audit.last_hit_rule.as_deref(),
        Some("DOMAIN-SUFFIX,google.com,Proxy")
    );

    // Trigger audit: indices are derived from the shared dead-rule projection,
    // not from any `idx % 2` fabrication.
    let _ = state.update(Message::AuditStaleRules);
    assert_eq!(
        state.editor.rule_hit_audit.zero_hit_rule_indices,
        vec![1, 3]
    );

    // Disable stale rules
    let _ = state.update(Message::DisableZeroHitRules);
    assert!(state.editor.rules[0].enabled);
    assert!(!state.editor.rules[1].enabled); // disabled rule 1
    assert!(state.editor.rules[2].enabled);
    assert!(!state.editor.rules[3].enabled); // disabled rule 3
    assert!(state.editor.rules_dirty);
}

#[test]
fn test_clear_rule_hit_counters_without_host_port_is_honest() {
    let (mut state, _) = AppState::new();

    // Hostless: no tracer port is composed, so the reset must not mutate the
    // shared audit into a fabricated success.
    let audit = RuleHitAuditSnapshot {
        total_hits: 7,
        can_clear: true,
        ..Default::default()
    };
    assert!(state.apply_shared_surface_snapshot(rules_page_with_hit_audit(audit)));

    let _ = state.update(Message::ClearRuleHitCounters);
    assert_eq!(state.editor.rule_hit_audit.audit.total_hits, 7);
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
    state.shell.demo = true;

    let options = TunStack::options();
    assert_eq!(options.len(), 4);
    assert!(
        options[..3]
            .iter()
            .all(|option| matches!(&option.availability, TunStackAvailability::Supported))
    );
    assert!(matches!(
        &options[3].availability,
        TunStackAvailability::ReferenceOnly { .. }
    ));

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
        state
            .runtime
            .tun_stack_config
            .probe_result_summary
            .as_deref(),
        Some("Optimal MTU: 1420 bytes")
    );

    state.shell.demo = false;
    let _ = state.update(Message::SetTunStack("lwip".to_string()));
    assert!(
        state
            .shell
            .error_msg
            .as_deref()
            .is_some_and(|message| message.contains("reference-only"))
    );
}

#[test]
fn test_tun_mtu_probe_result_updates_the_shared_iced_state() {
    let (mut state, _) = AppState::new();
    let result = MtuNegotiationSnapshot::ready(
        1,
        PhysicalMtuSnapshot {
            interface: "wlan0".to_owned(),
            mtu: 1500,
        },
        1420,
        1380,
    );
    let _ = state.update(Message::MtuProbeFinished(MtuProbeCompletion {
        snapshot: result,
        generation: state.runtime.runtime_generation,
        session_token: state.runtime.core_session_token,
    }));

    assert!(state.runtime.mtu.is_ready());
    assert_eq!(
        state.runtime.mtu.physical_interface.as_deref(),
        Some("wlan0")
    );
    assert_eq!(state.runtime.tun_stack_config.negotiated_mtu, 1420);
    assert!(
        state
            .runtime
            .tun_stack_config
            .probe_result_summary
            .as_deref()
            .is_some_and(|summary| summary.contains("wlan0"))
    );
}

#[test]
fn test_shared_surface_route_flags_update_the_iced_projection() {
    let (mut state, _) = AppState::new();
    let mut snapshot = SurfaceSnapshot::unavailable(
        SurfaceKind::IcedDesktop,
        HostKind::Desktop,
        Failure::new(ErrorCode::NotReady, "test snapshot", true),
    );
    snapshot.revision = 1;
    snapshot.pages.settings = PageData::ready(SettingsPageSnapshot {
        autostart: false,
        system_proxy: false,
        mixed_port: 7890,
        allow_lan: false,
        lan_bind_address: "*".to_owned(),
        lan_security: Default::default(),
        ipv6_routing: Ipv6RoutingSnapshot::new(2, false, true),
        pac: Default::default(),
        tun_enabled: true,
        tun_stack: "system".to_owned(),
        tun_auto_route: true,
        tun_strict_route: true,
        controller_port: 9090,
        log_level: "info".to_owned(),
        core_channel: "stable".to_owned(),
    });

    assert!(state.apply_shared_surface_snapshot(snapshot));
    assert!(state.editor.tun_auto_route);
    assert!(state.editor.tun_strict_route);
    assert!(!state.runtime.ipv6_routing.enabled);
    assert!(state.runtime.ipv6_routing.tun_enabled);
}

#[test]
fn test_shared_surface_system_proxy_updates_the_iced_projection() {
    let (mut state, _) = AppState::new();
    let mut snapshot = SurfaceSnapshot::unavailable(
        SurfaceKind::IcedDesktop,
        HostKind::Desktop,
        Failure::new(ErrorCode::NotReady, "test snapshot", true),
    );
    snapshot.revision = 1;
    snapshot.system_proxy =
        infiltrator_contract::system_proxy::SystemProxySnapshot::from_observation(
            1,
            SystemProxyObservation {
                enabled: true,
                endpoint: Some("127.0.0.1:7890".to_owned()),
                bypass: Some("localhost".to_owned()),
            },
        );

    assert!(state.apply_shared_surface_snapshot(snapshot));
    assert!(state.runtime.system_proxy_enabled);
    assert_eq!(
        state.runtime.system_proxy.status,
        SystemProxyStatus::Enabled
    );
}

#[test]
fn test_system_proxy_recovery_updates_the_iced_projection() {
    let (mut state, _) = AppState::new();
    let snapshot = SystemProxyRecoverySnapshot {
        status: SystemProxyRecoveryStatus::Restored {
            previous: SystemProxyObservation::default(),
            restored: SystemProxyObservation::default(),
        },
        revision: 7,
    };

    let _ = state.update(Message::SystemProxyRecoveryFinished(snapshot.clone()));

    assert_eq!(state.runtime.system_proxy_recovery, snapshot);
    assert!(matches!(
        state.runtime.system_proxy_recovery.status,
        SystemProxyRecoveryStatus::Restored { .. }
    ));
}

#[test]
fn test_advancement_w5_4_rule_provider_lifecycle_and_unpack() {
    let (mut state, _) = AppState::new();

    let initial_count = state.editor.rules.len();

    // Unpack provider
    let _ = state.update(Message::UnpackRuleProviderToCustom(
        "Apple-Provider".to_string(),
    ));
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
    assert_eq!(state.runtime.apply_guard.stage, ApplyTransactionStage::Idle);

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

#[test]
fn test_lan_sharing_readback_updates_the_iced_state() {
    let (mut state, _) = AppState::new();
    let generation = state.runtime.runtime_generation;
    let snapshot = LanSharingSnapshot::new(4, true, 8080, "192.168.1.10");

    let _ = state.update(Message::LanSharingSet(Ok(snapshot.clone()), generation));

    assert!(state.runtime.lan_sharing.allow_lan);
    assert_eq!(state.runtime.lan_sharing.mixed_port, 8080);
    assert_eq!(state.runtime.lan_sharing.bind_address, "192.168.1.10");
    assert_eq!(
        state.runtime.lan_sharing_committed.bind_address,
        "192.168.1.10"
    );
}

#[test]
fn test_lan_security_readback_updates_iced_acl_and_clears_password() {
    let (mut state, _) = AppState::new();
    let generation = state.runtime.runtime_generation;
    state.runtime.lan_security.auth_password = "temporary-secret".to_owned();
    let snapshot = LanSecuritySnapshot::new(
        6,
        vec!["192.168.1.0/24".to_owned()],
        vec!["192.168.1.10/32".to_owned()],
        vec!["127.0.0.0/8".to_owned()],
        true,
        1,
        Some("lan-user".to_owned()),
    );

    let _ = state.update(Message::LanSecuritySet(Ok(snapshot), generation));

    assert_eq!(state.runtime.lan_security.allowed_ips, "192.168.1.0/24");
    assert_eq!(state.runtime.lan_security.disallowed_ips, "192.168.1.10/32");
    assert_eq!(state.runtime.lan_security.auth_username, "lan-user");
    assert!(state.runtime.lan_security.authentication_enabled);
    assert!(state.runtime.lan_security.auth_password.is_empty());
    assert!(!state.runtime.lan_security_dirty);
}

#[test]
fn test_shared_surface_keeps_a_dirty_lan_draft_until_apply_result() {
    let (mut state, _) = AppState::new();
    state.runtime.lan_sharing.bind_address = "192.168.1.10".to_owned();
    state.runtime.lan_sharing_dirty = true;

    let mut snapshot = SurfaceSnapshot::unavailable(
        SurfaceKind::IcedDesktop,
        HostKind::Desktop,
        Failure::new(ErrorCode::NotReady, "test snapshot", true),
    );
    snapshot.revision = 1;
    snapshot.pages.settings = PageData::ready(SettingsPageSnapshot {
        autostart: false,
        system_proxy: false,
        mixed_port: 7890,
        allow_lan: false,
        lan_bind_address: "*".to_owned(),
        lan_security: LanSecuritySnapshot::new(
            8,
            vec!["192.168.0.0/16".to_owned()],
            vec!["192.168.1.10/32".to_owned()],
            vec!["127.0.0.0/8".to_owned()],
            true,
            1,
            Some("lan-user".to_owned()),
        ),
        ipv6_routing: Default::default(),
        pac: Default::default(),
        tun_enabled: false,
        tun_stack: String::new(),
        tun_auto_route: false,
        tun_strict_route: false,
        controller_port: 9090,
        log_level: "info".to_owned(),
        core_channel: "stable".to_owned(),
    });

    assert!(state.apply_shared_surface_snapshot(snapshot));
    assert_eq!(state.runtime.lan_sharing.bind_address, "192.168.1.10");
    assert_eq!(state.runtime.lan_sharing_committed.bind_address, "*");
    assert_eq!(state.runtime.lan_security.allowed_ips, "192.168.0.0/16");
    assert!(state.runtime.lan_security.authentication_enabled);
    assert_eq!(state.runtime.lan_security.auth_username, "lan-user");
}
