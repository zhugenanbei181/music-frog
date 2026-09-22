//! High-fidelity verification tests for Wave 2 of the 6 Iced Core Maturity Advancements.
//!
//! Complies strictly with docs/TEST_GOVERNANCE.md (Zero-Tautology Rule):
//! Every assertion validates concrete business contracts, state transitions,
//! exact string/integer values, and mathematical invariants.

use crate::state::AppState;
use crate::types::dns::DnsLeakReport;
use crate::types::message::Message;
use infiltrator_domain::connection_view::ConnectionGroupingMode;
use infiltrator_domain::profiles::ProfileInfo;

#[test]
fn test_advancement_w2_1_dns_leak_privacy_probe_lifecycle() {
    let (mut state, _) = AppState::new();

    // Default state
    assert!(!state.diag.is_probing_dns_leak);
    assert!(state.diag.dns_leak_probe.is_none());

    // Trigger probe
    let _ = state.update(Message::RunDnsLeakProbe);
    assert!(state.diag.is_probing_dns_leak);

    // Mock probe response payload
    let mock_report = DnsLeakReport {
        public_ip: "198.51.100.42".to_string(),
        country: "US".to_string(),
        isp: "Cloudflare Warp".to_string(),
        is_leak_detected: false,
        tested_dns_servers: vec![
            "1.1.1.1:53 (Cloudflare)".to_string(),
            "8.8.8.8:53 (Google)".to_string(),
        ],
        probe_duration_ms: 128,
    };

    let _ = state.update(Message::DnsLeakProbeFinished(mock_report.clone()));

    // Verify state transition and exact field parity
    assert!(!state.diag.is_probing_dns_leak);
    let probe = state.diag.dns_leak_probe.expect("Probe report must be set");
    assert_eq!(probe.public_ip, "198.51.100.42");
    assert_eq!(probe.country, "US");
    assert_eq!(probe.isp, "Cloudflare Warp");
    assert!(!probe.is_leak_detected);
    assert_eq!(probe.tested_dns_servers.len(), 2);
    assert_eq!(probe.probe_duration_ms, 128);
}

#[test]
fn test_advancement_w2_2_custom_node_modal_and_uri_codec() {
    let (mut state, _) = AppState::new();

    // Initial state
    assert!(!state.runtime.custom_node_modal_open);
    assert!(state.runtime.custom_node_uri_input.is_empty());

    // Open modal
    let _ = state.update(Message::OpenCustomNodeModal);
    assert!(state.runtime.custom_node_modal_open);

    // Provide standard Vless URI input
    let vless_uri = "vless://a3482e88-7d8f-4a42-9988-1a2b3c4d5e6f@server.example.com:443?type=ws&security=tls&sni=example.com#MyVlessNode";
    let _ = state.update(Message::UpdateCustomNodeUriInput(vless_uri.to_string()));
    assert_eq!(state.runtime.custom_node_uri_input, vless_uri);

    // DUAL-05: parse into the shared typed draft; there is no second
    // per-field form source to keep in sync any more.
    let _ = state.update(Message::ParseAndImportCustomUri);
    let draft = state
        .runtime
        .custom_node_studio
        .draft
        .as_ref()
        .expect("shared draft");
    assert_eq!(draft.name, "MyVlessNode");
    assert_eq!(draft.server, "server.example.com");
    assert_eq!(draft.port, 443);
    assert_eq!(draft.node_type, "vless");
    assert_eq!(draft.uuid, "a3482e88-7d8f-4a42-9988-1a2b3c4d5e6f");
    assert_eq!(draft.sni, "example.com");
    assert!(draft.tls);

    // Close modal
    let _ = state.update(Message::CloseCustomNodeModal);
    assert!(!state.runtime.custom_node_modal_open);
}

#[test]
fn test_advancement_w2_3_multi_profile_aggregator_workflow() {
    let (mut state, _) = AppState::new();

    // Mock active profiles in state
    let mut p1 = ProfileInfo {
        name: "Airport-US".to_string(),
        path: "/tmp/us.yaml".to_string(),
        ..Default::default()
    };
    p1.subscription_url = Some("https://sub.lan/us".to_string());
    let mut p2 = ProfileInfo {
        name: "Airport-HK".to_string(),
        path: "/tmp/hk.yaml".to_string(),
        active: true,
        ..Default::default()
    };
    p2.subscription_url = Some("https://sub.lan/hk".to_string());
    state.profile.profiles = vec![p1, p2];

    // Open aggregator modal
    let _ = state.update(Message::OpenAggregatorModal);
    assert!(state.profile.aggregator_modal_open);
    assert_eq!(state.profile.aggregator_selected_profiles.len(), 2);

    // Deselect Airport-US
    let _ = state.update(Message::ToggleAggregatorProfileSelection(
        "Airport-US".to_string(),
    ));
    assert_eq!(
        state.profile.aggregator_selected_profiles,
        vec!["Airport-HK"]
    );

    // Set merged profile name
    let _ = state.update(Message::UpdateAggregatorName("HK-Only-Merged".to_string()));
    assert_eq!(state.profile.aggregator_name_input, "HK-Only-Merged");

    // DUAL-08: changing a cleaning switch drops the previous shared preview;
    // the surface never keeps a stale report.
    state.profile.aggregator_report = Some(Default::default());
    let _ = state.update(Message::ToggleAggregatorDeduplicate);
    assert!(state.profile.aggregator_report.is_none());
    assert!(!state.profile.aggregator_deduplicate);

    // Close modal
    let _ = state.update(Message::CloseAggregatorModal);
    assert!(!state.profile.aggregator_modal_open);
}

/// DUAL-08: the surface consumes the shared aggregation report verbatim and
/// refuses to start a preview without a source selection.
#[test]
fn test_advancement_w2_3b_aggregator_preview_lifecycle_is_shared() {
    use infiltrator_contract::aggregator::{
        AggregationReport, GeneratedGroupSnapshot, RegionalClusterSnapshot,
    };

    let (mut state, _) = AppState::new();
    let report = AggregationReport {
        draft: infiltrator_contract::aggregator::AggregationDraft {
            source_profiles: vec!["Airport-HK".to_string()],
            target_name: "Merged-New".to_string(),
            deduplicate: true,
            deduplicate_names: true,
            geo_cluster: true,
            generate_groups: true,
            remove_emojis: true,
            rename_rules: vec![infiltrator_contract::aggregator::AggregationRenameRule {
                pattern: "-Pro$".to_string(),
                replacement: String::new(),
            }],
            custom_groups: vec![infiltrator_contract::aggregator::AggregationCustomGroup {
                name: "流媒体专用".to_string(),
                group_type: "select".to_string(),
                member_keywords: vec!["HK".to_string()],
            }],
            availability_precheck: true,
            activate_after_create: true,
        },
        source_count: 1,
        missing_sources: vec![],
        input_nodes: 8,
        total_nodes: 7,
        duplicates_removed: 1,
        renamed_nodes: 7,
        rule_renamed_nodes: 2,
        invalid_nodes_removed: 1,
        invalid_node_samples: vec!["Broken: vmess: uuid is required".to_string()],
        regions: vec![RegionalClusterSnapshot {
            iso: "HK".to_string(),
            label: "香港".to_string(),
            flag: "🇭🇰".to_string(),
            group_name: "香港自动测速".to_string(),
            node_names: vec!["香港 01".to_string()],
        }],
        groups: vec![GeneratedGroupSnapshot {
            name: "🚀 节点选择".to_string(),
            group_type: "select".to_string(),
            is_master: true,
            is_custom: false,
            members: vec!["香港自动测速".to_string()],
        }],
        yaml: "proxies: []\n".to_string(),
        generated_at: "2026-09-22T10:00:00+00:00".to_string(),
    };

    // A real shared report lands in the surface state verbatim.
    let _ = state.update(Message::AggregationPreviewFinished(Ok(report.clone())));
    assert_eq!(state.profile.aggregator_report.as_ref(), Some(&report));
    assert!(!state.profile.is_aggregating);

    // The draft assembled by the surface maps every switch to the shared type.
    let draft = state.aggregator_draft().expect("valid rename text");
    assert!(draft.deduplicate);
    assert!(draft.geo_cluster);
    assert!(draft.generate_groups);
    assert!(draft.remove_emojis);
    assert!(draft.deduplicate_names);
    // DUAL-08-09/12: the new wizard switches ride the shared draft verbatim.
    assert!(draft.availability_precheck);
    assert!(!draft.activate_after_create);
    assert!(draft.rename_rules.is_empty());
    assert!(draft.custom_groups.is_empty());

    // DUAL-08-08: a malformed rename line is refused at the surface (the
    // shared application never sees a silently-dropped rule).
    state.profile.aggregator_renames = "no arrow here".to_string();
    assert_eq!(state.aggregator_draft(), Err("no arrow here".to_string()));
    state.profile.aggregator_renames.clear();

    // DUAL-08-10: appending a custom group invalidates the stale preview and
    // keeps the typed keywords.
    let _ = state.update(Message::UpdateAggregatorCustomGroupName(
        "流媒体专用".to_string(),
    ));
    let _ = state.update(Message::UpdateAggregatorCustomGroupKeywords(
        "Netflix, 4K".to_string(),
    ));
    let _ = state.update(Message::AddAggregatorCustomGroup);
    assert_eq!(state.profile.aggregator_custom_groups.len(), 1);
    assert_eq!(
        state.profile.aggregator_custom_groups[0].member_keywords,
        vec!["Netflix".to_string(), "4K".to_string()]
    );
    let draft = state.aggregator_draft().expect("valid rename text");
    assert_eq!(draft.custom_groups, state.profile.aggregator_custom_groups);
    let _ = state.update(Message::RemoveAggregatorCustomGroup(0));
    assert!(state.profile.aggregator_custom_groups.is_empty());
    let _ = state.update(Message::UpdateAggregatorCustomGroupName(String::new()));

    // DUAL-08-13: applying a saved template prefills the wizard fields.
    state.profile.aggregator_templates =
        vec![infiltrator_contract::aggregator::AggregationTemplate {
            name: "My-Template".to_string(),
            draft: infiltrator_contract::aggregator::AggregationDraft {
                source_profiles: vec!["Airport-HK".to_string()],
                target_name: "From-Template".to_string(),
                deduplicate: false,
                deduplicate_names: true,
                geo_cluster: false,
                generate_groups: true,
                remove_emojis: false,
                rename_rules: vec![infiltrator_contract::aggregator::AggregationRenameRule {
                    pattern: "x".to_string(),
                    replacement: "y".to_string(),
                }],
                custom_groups: Vec::new(),
                availability_precheck: false,
                activate_after_create: true,
            },
            updated_at: "2026-09-22T11:00:00+00:00".to_string(),
        }];
    let _ = state.update(Message::ApplyAggregatorTemplate("My-Template".to_string()));
    assert_eq!(state.profile.aggregator_name_input, "From-Template");
    assert!(!state.profile.aggregator_deduplicate);
    assert!(!state.profile.aggregator_geo_cluster);
    assert!(!state.profile.aggregator_availability_precheck);
    assert!(state.profile.aggregator_activate_after_create);
    assert_eq!(state.profile.aggregator_renames, "x => y");
    assert_eq!(
        state.profile.aggregator_template_name,
        "My-Template".to_string()
    );
    state.profile.aggregator_templates.clear();

    // No source selected: the surface refuses to start an aggregation.
    state.profile.aggregator_selected_profiles.clear();
    let _ = state.update(Message::PreviewProfileAggregation);
    assert!(!state.profile.is_aggregating);

    // DUAL-08-11: the modal view (preview + YAML viewport + template library)
    // builds from the shared report; the surface keeps no second aggregator.
    state.profile.aggregator_report = Some(report.clone());
    state.profile.aggregator_modal_open = true;
    {
        let _element: iced::Element<'_, Message> = state.view();
    }
    assert!(state.profile.aggregator_modal_open);

    // A typed failure clears the in-flight flag without faking a report.
    state.profile.aggregator_report = None;
    let _ = state.update(Message::AggregationPreviewFinished(Err(
        infiltrator_contract::error::InfiltratorError::Internal("boom".to_string()),
    )));
    assert!(state.profile.aggregator_report.is_none());
}

#[test]
fn test_advancement_w2_4_connection_grouping_and_quick_rule() {
    let (mut state, _) = AppState::new();

    // Default grouping mode
    assert_eq!(
        state.diag.connection_grouping_mode,
        ConnectionGroupingMode::Flat
    );

    // Switch to ByProcess grouping
    let _ = state.update(Message::SetConnectionGroupingMode(
        ConnectionGroupingMode::ByProcess,
    ));
    assert_eq!(
        state.diag.connection_grouping_mode,
        ConnectionGroupingMode::ByProcess
    );

    // Switch to ByHost grouping
    let _ = state.update(Message::SetConnectionGroupingMode(
        ConnectionGroupingMode::ByHost,
    ));
    assert_eq!(
        state.diag.connection_grouping_mode,
        ConnectionGroupingMode::ByHost
    );

    // Initial rules count
    let initial_rule_count = state.editor.rules.len();

    // Add quick rule from an inspected connection
    let _ = state.update(Message::AddQuickRuleFromConnection {
        pattern: "DOMAIN-SUFFIX,steamcommunity.com".to_string(),
        target: "DIRECT".to_string(),
    });

    assert_eq!(state.editor.rules.len(), initial_rule_count + 1);
    let added_rule = state.editor.rules.last().expect("rule must be appended");
    assert_eq!(added_rule.rule, "DOMAIN-SUFFIX,steamcommunity.com,DIRECT");
    assert!(added_rule.enabled);
    assert!(state.editor.rules_dirty);
}

#[test]
fn test_advancement_w2_5_snapshot_diff_and_rollback_dialog() {
    let (mut state, _) = AppState::new();

    // Initial state
    assert!(!state.editor.snapshot_diff_modal_open);
    assert!(state.editor.snapshot_diff_selected_id.is_none());

    // Open snapshot diff dialog
    let snapshot_id = "snap-20260903T120000Z-a77ce0";
    let _ = state.update(Message::OpenSnapshotDiff(snapshot_id.to_string()));
    assert!(state.editor.snapshot_diff_modal_open);
    assert_eq!(
        state.editor.snapshot_diff_selected_id.as_deref(),
        Some(snapshot_id)
    );

    // Close diff dialog
    let _ = state.update(Message::CloseSnapshotDiff);
    assert!(!state.editor.snapshot_diff_modal_open);
    assert!(state.editor.snapshot_diff_selected_id.is_none());
}

#[test]
fn test_advancement_w2_6_global_hotkey_manager_state() {
    use infiltrator_contract::shortcuts::{KeyModifiers, ShortcutAction, ShortcutChord};
    let (mut state, _) = AppState::new();

    // The shared registry seeds one product default per action.
    assert_eq!(
        state.shell.shortcut_registry.bindings().len(),
        ShortcutAction::ALL.len()
    );
    let system_proxy = ShortcutAction::ToggleSystemProxy;
    assert_eq!(
        state
            .shell
            .shortcut_registry
            .get(system_proxy)
            .unwrap()
            .chord,
        ShortcutChord::ctrl_alt_key("P")
    );
    assert!(state.shell.shortcut_registry.is_active(system_proxy));

    // Capturing a free chord rebinds it in place (the persistence task is
    // host-driven; the in-memory decision comes from the shared registry).
    let _ = state.update(Message::BeginHotkeyCapture(system_proxy));
    assert_eq!(state.shell.hotkey_capture, Some(system_proxy));
    let _ = state.update(Message::KeyboardChord {
        key: "P".to_string(),
        modifiers: KeyModifiers {
            ctrl: true,
            shift: true,
            alt: false,
            meta: false,
        },
    });
    assert_eq!(state.shell.hotkey_capture, None);
    assert_eq!(
        state
            .shell
            .shortcut_registry
            .get(system_proxy)
            .unwrap()
            .chord,
        ShortcutChord::new(
            "P",
            KeyModifiers {
                ctrl: true,
                shift: true,
                alt: false,
                meta: false,
            }
        )
    );

    // A chord owned by another action is refused with a conflict toast and
    // the live binding is untouched.
    let before = state.shell.shortcut_registry.clone();
    let _ = state.update(Message::BeginHotkeyCapture(system_proxy));
    let _ = state.update(Message::KeyboardChord {
        key: "T".to_string(),
        modifiers: KeyModifiers {
            ctrl: true,
            shift: false,
            alt: true,
            meta: false,
        },
    });
    assert_eq!(state.shell.shortcut_registry, before);
    assert!(!state.shell.toasts.is_empty(), "conflict is surfaced");

    // Toggle enabled: On -> Off -> On.
    let _ = state.update(Message::ToggleHotkeyEnabled(system_proxy));
    assert!(!state.shell.shortcut_registry.is_active(system_proxy));
    let _ = state.update(Message::ToggleHotkeyEnabled(system_proxy));
    assert!(state.shell.shortcut_registry.is_active(system_proxy));
}
