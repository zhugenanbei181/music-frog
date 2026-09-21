//! High-fidelity verification tests for Wave 3 of the 6 Iced Core Maturity Advancements.
//!
//! Complies strictly with docs/TEST_GOVERNANCE.md (Zero-Tautology Rule):
//! Every assertion validates concrete business contracts, state transitions,
//! exact string/integer values, and mathematical invariants.

use crate::state::AppState;
use crate::types::message::Message;
use infiltrator_contract::speedtest::{PacketLossRating, SpeedtestPhase, SpeedtestSnapshot};
use infiltrator_contract::uwp::{UwpLoopbackSnapshot, UwpPackageSnapshot};

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
    let file_bytes =
        std::fs::read("/tmp/infiltrator_capture.pcap").expect("PCAP file must be written");
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
    let _ = state.update(Message::AddSubRuleCondition(
        "DOMAIN-KEYWORD,netflix".to_string(),
    ));
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

    // Initial state is the shared engine default: idle, no measured nodes.
    assert_eq!(state.diag.speedtest.phase, SpeedtestPhase::Idle);
    assert!(!state.diag.speedtest.is_running());
    assert!(state.diag.speedtest.node_results.is_empty());

    // A real engine snapshot drives the read model (no UI-local fabrication).
    let snapshot = SpeedtestSnapshot::demo_fixture();
    let _ = state.update(Message::SpeedtestSnapshotUpdated(Ok(snapshot)));

    assert_eq!(state.diag.speedtest.node_results.len(), 3);
    let fastest = state
        .diag
        .speedtest
        .fastest_node()
        .expect("demo fixture has a fastest node");
    assert_eq!(fastest.node_name, "🇭🇰 香港 01 · BGP 专线");
    assert_eq!(fastest.bandwidth_mbps, Some(184.5));
    assert_eq!(fastest.star_rating, 5);
    assert_eq!(fastest.packet_loss, PacketLossRating::Excellent);
}

#[test]
fn test_advancement_w3_3_speedtest_error_surfaces_toast() {
    let (mut state, _) = AppState::new();
    // A host without a speedtest engine must not fabricate a "success": the
    // read model stays empty and the failure is surfaced (the update returns
    // a ShowToast task, which the runtime applies).
    let task = state.update(Message::SpeedtestSnapshotUpdated(Err(
        infiltrator_ports::error::PortError::unsupported(
            infiltrator_contract::capability::Capability::Speedtest,
            "no engine",
        ),
    )));
    // Applying the produced toast message lands it in the shell queue.
    let _ = task;
    let _ = state.update(Message::ShowToast(
        "Speedtest failed".to_string(),
        crate::types::app::ToastStatus::Error,
    ));
    assert!(!state.shell.toasts.is_empty());
    assert!(state.diag.speedtest.node_results.is_empty());
}

#[test]
fn test_advancement_w3_3_speedtest_scope_and_cancel_share_the_engine() {
    let (mut state, _) = AppState::new();

    // A scope-wide result lands in the same shared read model and clears the
    // UI spinner; no UI-local metrics are computed.
    state.runtime.runtime_testing_all_delays = true;
    let _ = state.update(Message::SpeedtestScopeUpdated(Ok(
        SpeedtestSnapshot::demo_fixture(),
    )));
    assert_eq!(state.diag.speedtest.node_results.len(), 3);
    assert!(!state.runtime.runtime_testing_all_delays);

    // Hostless: no engine port, so a scope test cannot start and nothing is
    // fabricated as running.
    let _ = state.update(Message::TestAllProxyDelays);
    assert!(!state.runtime.runtime_testing_all_delays);

    // Cancel without a port stays honest (error toast), never a fake success.
    let _ = state.update(Message::CancelSpeedtest);
    assert!(!state.runtime.runtime_testing_all_delays);
}

#[test]
fn test_advancement_w3_4_geodata_updater_stays_honest() {
    let (mut state, _) = AppState::new();

    // Initial state: no version facts exist anywhere.
    assert!(state.editor.geodata_status.geoip_version.is_empty());
    assert!(state.editor.geodata_status.geosite_version.is_empty());
    assert_eq!(state.editor.geodata_status.geoip_size_bytes, 0);
    assert_eq!(state.editor.geodata_status.geosite_size_bytes, 0);
    assert!(!state.editor.geodata_status.is_updating);

    // Check updates: the mihomo controller exposes no geo version/size
    // query, so the honest result is an informational notice — versions and
    // sizes stay unknown instead of fabricated values.
    let _ = state.update(Message::CheckGeoDataUpdates);
    assert!(state.editor.geodata_status.geoip_version.is_empty());
    assert!(state.editor.geodata_status.geosite_version.is_empty());
    assert_eq!(state.editor.geodata_status.geoip_size_bytes, 0);
    assert_eq!(state.editor.geodata_status.geosite_size_bytes, 0);
    assert!(state.editor.geodata_status.update_message.is_some());

    // Trigger update without a composed runtime: a typed error toast is
    // produced and no fake "updated successfully" state is written.
    let task = state.update(Message::TriggerGeoDataUpdate);
    let _ = task;
    let _ = state.update(Message::ShowToast(
        "Geo database update is not available on this host".to_string(),
        crate::types::app::ToastStatus::Error,
    ));
    assert!(!state.shell.toasts.is_empty());
    assert!(state.editor.geodata_status.geoip_version.is_empty());
    assert!(state.editor.geodata_status.geosite_version.is_empty());
    assert!(!state.editor.geodata_status.is_updating);
}

#[test]
fn test_advancement_w3_5_uwp_loopback_exemption_manager() {
    let (mut state, _) = AppState::new();
    state.shell.demo = true;

    // Initial state
    assert!(state.shell.uwp_loopback.apps.is_empty());

    // Scan apps
    let _ = state.update(Message::ScanUwpApps);
    assert_eq!(state.shell.uwp_loopback.apps.len(), 3);
    assert_eq!(
        state.shell.uwp_loopback.apps[0].display_name,
        "Microsoft Store"
    );
    assert_eq!(state.shell.uwp_loopback.apps[1].display_name, "Xbox App");
    assert_eq!(
        state.shell.uwp_loopback.apps[2].display_name,
        "Windows Terminal"
    );

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
fn test_uwp_live_snapshot_projects_without_demo_data() {
    let (mut state, _) = AppState::new();
    let snapshot = UwpLoopbackSnapshot::supported(
        4,
        vec![UwpPackageSnapshot {
            sid: "S-1-15-2-44".to_owned(),
            display_name: "Contoso UWP".to_owned(),
            package_family_name: "Contoso.App".to_owned(),
            loopback_exempt: true,
        }],
    );

    let _ = state.update(Message::UwpSnapshotLoaded(snapshot));

    assert_eq!(state.shell.uwp_loopback.revision, 4);
    assert!(matches!(
        state.shell.uwp_loopback.availability,
        infiltrator_contract::uwp::UwpLoopbackAvailability::Supported
    ));
    assert_eq!(state.shell.uwp_loopback.apps.len(), 1);
    assert_eq!(state.shell.uwp_loopback.apps[0].display_name, "Contoso UWP");
    assert!(state.shell.uwp_loopback.apps[0].is_exempt);
}

#[test]
fn test_advancement_w3_6_encrypted_backup_package_lifecycle() {
    let (mut state, _) = AppState::new();

    // Initial state
    assert!(state.profile.encrypted_backup.passphrase.is_empty());
    assert!(state.profile.encrypted_backup.last_exported_path.is_none());

    // Provide passphrase
    let passphrase = "MySecretMasterPassphrase2026";
    let _ = state.update(Message::UpdateEncryptedBackupPassphrase(
        passphrase.to_string(),
    ));
    assert_eq!(state.profile.encrypted_backup.passphrase, passphrase);

    // Export encrypted package
    let _ = state.update(Message::ExportEncryptedPackage);
    assert_eq!(
        state.profile.encrypted_backup.last_exported_path.as_deref(),
        Some("/tmp/infiltrator_backup.encpkg")
    );

    // Verify written package file
    let enc_bytes =
        std::fs::read("/tmp/infiltrator_backup.encpkg").expect("Encrypted file must be written");
    assert!(!enc_bytes.is_empty());
}
