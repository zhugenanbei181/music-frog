//! High-fidelity verification tests for Wave 3 of the 6 Iced Core Maturity Advancements.
//!
//! Complies strictly with docs/TEST_GOVERNANCE.md (Zero-Tautology Rule):
//! Every assertion validates concrete business contracts, state transitions,
//! exact string/integer values, and mathematical invariants.

use crate::state::AppState;
use crate::types::message::Message;
use crate::types::perf::SpeedtestResult;

#[test]
fn test_advancement_w3_1_pcap_capture_and_export_lifecycle() {
    let (mut state, _) = AppState::new();

    // Default state: not capturing
    assert!(!state.diag.pcap_state.is_capturing);
    assert_eq!(state.diag.pcap_state.packet_count, 0);
    assert!(state.diag.pcap_state.exported_path.is_none());

    // Start capture
    let _ = state.update(Message::TogglePcapCapture);
    assert!(state.diag.pcap_state.is_capturing);

    // Simulate captured packets
    state.diag.pcap_state.packet_count = 142;
    state.diag.pcap_state.total_bytes = 65536;

    // Stop capture
    let _ = state.update(Message::TogglePcapCapture);
    assert!(!state.diag.pcap_state.is_capturing);

    // Export PCAP file
    let _ = state.update(Message::ExportPcapBuffer);
    assert_eq!(
        state.diag.pcap_state.exported_path.as_deref(),
        Some("/tmp/infiltrator_capture.pcap")
    );

    // Verify written file exists on disk
    let file_bytes = std::fs::read("/tmp/infiltrator_capture.pcap").expect("PCAP file must be written");
    assert!(file_bytes.len() >= 24); // PCAP global header is 24 bytes
}

#[test]
fn test_advancement_w3_2_subrules_logical_builder_workflow() {
    let (mut state, _) = AppState::new();

    // Default draft state
    assert_eq!(state.editor.subrule_draft.operator, "AND");
    assert_eq!(state.editor.subrule_draft.target, "DIRECT");

    // Update operator to OR
    let _ = state.update(Message::UpdateSubRuleOperator("OR".to_string()));
    assert_eq!(state.editor.subrule_draft.operator, "OR");

    // Add condition
    let _ = state.update(Message::AddSubRuleCondition("DOMAIN-KEYWORD,netflix".to_string()));
    assert_eq!(state.editor.subrule_draft.conditions.len(), 3);
    assert_eq!(
        state.editor.subrule_draft.conditions[2],
        "DOMAIN-KEYWORD,netflix"
    );

    // Remove first condition
    let _ = state.update(Message::RemoveSubRuleCondition(0));
    assert_eq!(state.editor.subrule_draft.conditions.len(), 2);
    assert_eq!(state.editor.subrule_draft.conditions[0], "NETWORK,TCP");

    // Update target
    let _ = state.update(Message::UpdateSubRuleTarget("StreamingGroup".to_string()));
    assert_eq!(state.editor.subrule_draft.target, "StreamingGroup");

    // Insert into rules
    let initial_rule_count = state.editor.rules.len();
    let _ = state.update(Message::InsertSubRuleIntoRules);

    assert_eq!(state.editor.rules.len(), initial_rule_count + 1);
    let inserted = state.editor.rules.last().expect("rule must be inserted");
    assert_eq!(
        inserted.rule,
        "OR((NETWORK,TCP, DOMAIN-KEYWORD,netflix)),StreamingGroup"
    );
    assert!(inserted.enabled);
    assert!(state.editor.rules_dirty);
}

#[test]
fn test_advancement_w3_3_speedtest_and_jitter_benchmark_result() {
    let (mut state, _) = AppState::new();

    // Initial state
    assert_eq!(state.diag.speedtest_result.bandwidth_mbps, 0.0);
    assert!(!state.diag.speedtest_result.is_running);

    // Mock speedtest completion
    let res = SpeedtestResult {
        target_node: "HK-BGP-01".to_string(),
        bandwidth_mbps: 184.5,
        jitter_ms: 2.8,
        packet_loss_percent: 0.0,
        tier: "Excellent".to_string(),
        is_running: false,
    };

    let _ = state.update(Message::NodeSpeedtestFinished(res));

    assert_eq!(state.diag.speedtest_result.target_node, "HK-BGP-01");
    assert_eq!(state.diag.speedtest_result.bandwidth_mbps, 184.5);
    assert_eq!(state.diag.speedtest_result.jitter_ms, 2.8);
    assert_eq!(state.diag.speedtest_result.packet_loss_percent, 0.0);
    assert_eq!(state.diag.speedtest_result.tier, "Excellent");
    assert!(!state.diag.speedtest_result.is_running);
}

#[test]
fn test_advancement_w3_4_geodata_version_and_updater_workflow() {
    let (mut state, _) = AppState::new();

    // Initial state
    assert!(state.editor.geodata_status.geoip_version.is_empty());
    assert!(!state.editor.geodata_status.is_updating);

    // Check updates
    let _ = state.update(Message::CheckGeoDataUpdates);
    assert_eq!(state.editor.geodata_status.geoip_version, "v2026.09.01");
    assert_eq!(state.editor.geodata_status.geosite_version, "v2026.09.01");
    assert!(state.editor.geodata_status.geoip_size_bytes > 0);
    assert!(state.editor.geodata_status.geosite_size_bytes > 0);

    // Trigger update
    let _ = state.update(Message::TriggerGeoDataUpdate);
    assert_eq!(state.editor.geodata_status.geoip_version, "v2026.09.03");
    assert_eq!(state.editor.geodata_status.geosite_version, "v2026.09.03");
    assert_eq!(
        state.editor.geodata_status.update_message.as_deref(),
        Some("Updated GeoIP and GeoSite successfully")
    );
}

#[test]
fn test_advancement_w3_5_uwp_loopback_exemption_manager() {
    let (mut state, _) = AppState::new();

    // Initial state
    assert!(state.shell.uwp_loopback.apps.is_empty());

    // Scan apps
    let _ = state.update(Message::ScanUwpApps);
    assert_eq!(state.shell.uwp_loopback.apps.len(), 3);
    assert_eq!(state.shell.uwp_loopback.apps[0].display_name, "Microsoft Store");
    assert_eq!(state.shell.uwp_loopback.apps[1].display_name, "Xbox App");
    assert_eq!(state.shell.uwp_loopback.apps[2].display_name, "Windows Terminal");

    // Exempt all
    let _ = state.update(Message::ExemptAllUwpApps);
    for app in &state.shell.uwp_loopback.apps {
        assert!(app.is_exempt);
    }

    // Clear all exemptions
    let _ = state.update(Message::ClearAllUwpExemptions);
    for app in &state.shell.uwp_loopback.apps {
        assert!(!app.is_exempt);
    }

    // Toggle single app
    let _ = state.update(Message::ToggleUwpAppExemption("S-1-15-2-1".to_string()));
    assert!(state.shell.uwp_loopback.apps[0].is_exempt);
    assert!(!state.shell.uwp_loopback.apps[1].is_exempt);
}

#[test]
fn test_advancement_w3_6_encrypted_backup_package_lifecycle() {
    let (mut state, _) = AppState::new();

    // Initial state
    assert!(state.profile.encrypted_backup.passphrase.is_empty());
    assert!(state.profile.encrypted_backup.last_exported_path.is_none());

    // Provide passphrase
    let passphrase = "MySecretMasterPassphrase2026";
    let _ = state.update(Message::UpdateEncryptedBackupPassphrase(passphrase.to_string()));
    assert_eq!(state.profile.encrypted_backup.passphrase, passphrase);

    // Export encrypted package
    let _ = state.update(Message::ExportEncryptedPackage);
    assert_eq!(
        state.profile.encrypted_backup.last_exported_path.as_deref(),
        Some("/tmp/infiltrator_backup.encpkg")
    );

    // Verify written package file
    let enc_bytes = std::fs::read("/tmp/infiltrator_backup.encpkg").expect("Encrypted file must be written");
    assert!(!enc_bytes.is_empty());
}
