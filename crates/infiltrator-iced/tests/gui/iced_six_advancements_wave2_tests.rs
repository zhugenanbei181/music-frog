//! High-fidelity verification tests for Wave 2 of the 6 Iced Core Maturity Advancements.
//!
//! Complies strictly with docs/TEST_GOVERNANCE.md (Zero-Tautology Rule):
//! Every assertion validates concrete business contracts, state transitions,
//! exact string/integer values, and mathematical invariants.

use crate::state::AppState;
use crate::types::dns::DnsLeakReport;
use crate::types::message::Message;
use crate::types::runtime::ConnectionGroupingMode;
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

    // Parse URI into form fields
    let _ = state.update(Message::ParseAndImportCustomUri);
    assert_eq!(state.runtime.custom_node_name_input, "MyVlessNode");
    assert_eq!(state.runtime.custom_node_server_input, "server.example.com");
    assert_eq!(state.runtime.custom_node_port_input, "443");
    assert_eq!(state.runtime.custom_node_type_input, "vless");
    assert_eq!(
        state.runtime.custom_node_uuid_input,
        "a3482e88-7d8f-4a42-9988-1a2b3c4d5e6f"
    );
    assert_eq!(state.runtime.custom_node_sni_input, "example.com");

    // Close modal
    let _ = state.update(Message::CloseCustomNodeModal);
    assert!(!state.runtime.custom_node_modal_open);
}

#[test]
fn test_advancement_w2_3_multi_profile_aggregator_workflow() {
    let (mut state, _) = AppState::new();

    // Mock active profiles in state
    let mut p1 = ProfileInfo { name: "Airport-US".to_string(), path: "/tmp/us.yaml".to_string(), ..Default::default() };
    p1.subscription_url = Some("https://sub.lan/us".to_string());
    let mut p2 = ProfileInfo { name: "Airport-HK".to_string(), path: "/tmp/hk.yaml".to_string(), active: true, ..Default::default() };
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

    // Execute merge
    let _ = state.update(Message::ExecuteProfileAggregation);
    assert_eq!(
        state.profile.aggregator_result_summary.as_deref(),
        Some("Merged 1 profiles into 'HK-Only-Merged'")
    );

    // Close modal
    let _ = state.update(Message::CloseAggregatorModal);
    assert!(!state.profile.aggregator_modal_open);
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
    let (mut state, _) = AppState::new();

    // Verify default bindings
    assert_eq!(state.shell.hotkeys_config.len(), 3);
    assert_eq!(state.shell.hotkeys_config[0].id, "system_proxy");
    assert_eq!(state.shell.hotkeys_config[0].combo, "Ctrl+Alt+P");
    assert!(state.shell.hotkeys_config[0].enabled);

    // Update shortcut combo
    let _ = state.update(Message::UpdateHotkeyCombo {
        id: "system_proxy".to_string(),
        combo: "Ctrl+Shift+P".to_string(),
    });
    assert_eq!(state.shell.hotkeys_config[0].combo, "Ctrl+Shift+P");

    // Toggle shortcut enabled: On -> Off -> On
    let _ = state.update(Message::ToggleHotkeyEnabled("system_proxy".to_string()));
    assert!(!state.shell.hotkeys_config[0].enabled);

    let _ = state.update(Message::ToggleHotkeyEnabled("system_proxy".to_string()));
    assert!(state.shell.hotkeys_config[0].enabled);
}
