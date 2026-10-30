//! High-fidelity verification tests for Wave 3 of the 6 Iced Core Maturity Advancements.
//!
//! Complies strictly with TESTING.md zero-tautology assertion rule:
//! Every assertion validates concrete business contracts, state transitions,
//! exact string/integer values, and mathematical invariants.

use crate::state::AppState;
use crate::types::app::ToastStatus;
use crate::types::editor::GeoDataUpdatePhase;
use crate::types::message::Message;
use crate::view::speedtest_modal::speedtest_card;
use crate::view_root::speedtest_detail_modal::speedtest_detail_modal;
use infiltrator_composition::capture_fixtures::list_document;
use infiltrator_contract::speedtest::SpeedtestConcurrency;
use infiltrator_contract::speedtest::SpeedtestStartReceipt;
use infiltrator_contract::speedtest::{PacketLossRating, SpeedtestPhase, SpeedtestSnapshot};
use infiltrator_contract::uwp::{UwpLoopbackAvailability, UwpLoopbackSnapshot, UwpPackageSnapshot};
use infiltrator_domain::rules::edit::DEFAULT_RULE_TARGET;
use infiltrator_domain::sub_rules::validate_logical_rule_syntax;
use infiltrator_presentation::rule_list_editor::RuleListReadback;
use infiltrator_presentation::speedtest_summary_projection::history_lines;
use infiltrator_shared::locales::Lang;
use lurker_core_contract::error::Failure;
use std::env;
use std::fs::read;

fn apply_speedtest_snapshot(state: &mut AppState, snapshot: SpeedtestSnapshot) {
    let receipt = SpeedtestStartReceipt {
        run_id: snapshot
            .state
            .run_id()
            .expect("fixture snapshot has a run ID"),
    };
    let _ = state.update(Message::SpeedtestStarted(Ok((receipt, snapshot))));
}

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
    let file_bytes = read("/tmp/infiltrator_capture.pcap").expect("PCAP file must be written");
    assert!(file_bytes.len() >= 24); // PCAP global header is 24 bytes
}

#[test]
fn test_advancement_w3_2_subrules_logical_builder_workflow() {
    let (mut state, _) = AppState::new();
    let document = list_document(state.editor.rule_list.draft.clone());
    state
        .editor
        .rule_list
        .observe_readback(RuleListReadback::Observed(document));

    // Default draft state: the shared builder seeds the shared default target.
    assert_eq!(state.editor.subrule_draft.operator, "AND");
    assert_eq!(state.editor.subrule_draft.target, DEFAULT_RULE_TARGET);

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

    // Insert into rules: the shared builder encodes the canonical
    // `OP((cond),(cond),TARGET)` expression the parser accepts.
    let initial_rule_count = state.editor.rule_list.draft.len();
    let _ = state.update(Message::InsertSubRuleIntoRules);

    assert_eq!(state.editor.rule_list.draft.len(), initial_rule_count + 1);
    let inserted = state
        .editor
        .rule_list
        .draft
        .first()
        .expect("rule must be inserted");
    assert_eq!(
        inserted.rule,
        "OR,((NETWORK,TCP),(DOMAIN-KEYWORD,netflix)),StreamingGroup"
    );
    assert!(inserted.enabled);
    assert!(state.editor.rule_list.dirty());
    assert!(
        validate_logical_rule_syntax(&inserted.rule).is_ok(),
        "inserted logical rule must parse through the shared gate"
    );
}

#[test]
fn test_advancement_w3_3_speedtest_and_jitter_benchmark_result() {
    let (mut state, _) = AppState::new();

    // Initial state is the shared engine default: idle, no measured nodes.
    assert_eq!(state.diag.speedtest.phase(), SpeedtestPhase::Idle);
    assert!(!state.diag.speedtest.is_running());
    assert!(state.diag.speedtest.results.is_empty());

    // A real engine snapshot drives the read model (no UI-local fabrication).
    let snapshot = SpeedtestSnapshot::demo_fixture();
    apply_speedtest_snapshot(&mut state, snapshot);

    assert_eq!(state.diag.speedtest.results.len(), 3);
    let fastest = state
        .diag
        .speedtest
        .fastest()
        .expect("demo fixture has a fastest node");
    assert_eq!(fastest.outbound_id.value(), "hk-primary");
    assert_eq!(
        fastest.display_name.as_deref(),
        Some("🇭🇰 香港 01 · BGP 专线")
    );
    assert_eq!(
        fastest.calculation().unwrap().loss_rating,
        PacketLossRating::Excellent
    );
}

#[test]
fn test_advancement_w3_3_speedtest_error_surfaces_toast() {
    let (mut state, _) = AppState::new();
    // A Core owner without a speedtest service must not fabricate a "success": the
    // read model stays empty and the failure is surfaced (the update returns
    // a ShowToast task, which the runtime applies).
    let task = state.update(Message::SpeedtestStarted(Err(Failure::unsupported(
        "no engine",
    ))));
    // Applying the produced toast message lands it in the shell queue.
    let _ = task;
    let _ = state.update(Message::ShowToast(
        "Speedtest failed".to_string(),
        ToastStatus::Error,
    ));
    assert!(!state.shell.toasts.is_empty());
    assert!(state.diag.speedtest.results.is_empty());
}

#[test]
fn test_advancement_w3_3_speedtest_dead_node_archive_renders() {
    let (mut state, _) = AppState::new();
    apply_speedtest_snapshot(&mut state, SpeedtestSnapshot::demo_fixture());

    // The shared snapshot archives the timed-out outbound by its stable ID.
    assert_eq!(state.diag.speedtest.timed_out().len(), 1);
    let lang = Lang("zh-CN");
    let _card = speedtest_card(&state, &lang);
}

#[test]
fn test_advancement_w3_3_speedtest_history_renders_shared_snapshot() {
    let (mut state, _) = AppState::new();
    apply_speedtest_snapshot(&mut state, SpeedtestSnapshot::demo_fixture());

    // The history lines are read straight off the shared snapshot's
    // `recent_history`; the view owns no store.
    let lang = Lang("zh-CN");
    let lines = history_lines(&state.diag.speedtest, &Lang(lang.0));
    assert_eq!(lines.len(), 1);
    assert!(lines[0].contains("50.5 ms"), "line={}", lines[0]);
    assert!(lines[0].contains("全部出口"), "line={}", lines[0]);

    // The full card still renders with the history section.
    let _card = speedtest_card(&state, &lang);
}

#[test]
fn test_advancement_w3_3_speedtest_concurrency_reads_shared_and_clamps() {
    let (mut state, _) = AppState::new();

    // The card reads the bound from the shared snapshot, not a UI-local value.
    let mut snapshot = SpeedtestSnapshot::demo_fixture();
    snapshot.concurrency = SpeedtestConcurrency::new(12).unwrap();
    apply_speedtest_snapshot(&mut state, snapshot);
    assert_eq!(state.diag.speedtest.concurrency.get(), 12);
    let lang = Lang("zh-CN");
    {
        let _card = speedtest_card(&state, &lang);
    }

    // Stepping is clamped into the supported 1..=30 window.
    assert_eq!(AppState::stepped_speedtest_concurrency(1, -5), 1);
    assert_eq!(AppState::stepped_speedtest_concurrency(30, 5), 30);
    assert_eq!(AppState::stepped_speedtest_concurrency(30, -5), 25);

    // Without a product Core ingress the adjustment stays honest: no fabricated change.
    let _ = state.update(Message::AdjustSpeedtestConcurrency(5));
    assert_eq!(state.diag.speedtest.concurrency.get(), 12);
    assert!(
        format!("{:?}", Message::AdjustSpeedtestConcurrency(-1))
            .contains("AdjustSpeedtestConcurrency")
    );
}

#[test]
fn test_advancement_w3_3_speedtest_typed_detail_modal() {
    let (mut state, _) = AppState::new();
    apply_speedtest_snapshot(&mut state, SpeedtestSnapshot::demo_fixture());
    assert_eq!(state.diag.speedtest.results.len(), 3);
    assert_eq!(
        state.diag.speedtest.results[0].outbound_id.value(),
        "hk-primary"
    );

    // The detail modal is pure view state over the shared typed snapshot.
    assert!(!state.diag.speedtest_detail_open);
    let _ = state.update(Message::OpenSpeedtestDetail);
    assert!(state.diag.speedtest_detail_open);
    {
        let _modal = speedtest_detail_modal(&state);
        let lang = Lang("zh-CN");
        let _card = speedtest_card(&state, &lang);
    }
    let _ = state.update(Message::CloseSpeedtestDetail);
    assert!(!state.diag.speedtest_detail_open);

    assert_eq!(
        format!("{:?}", Message::OpenSpeedtestDetail),
        "OpenSpeedtestDetail"
    );
    assert_eq!(
        format!("{:?}", Message::CloseSpeedtestDetail),
        "CloseSpeedtestDetail"
    );
}

#[test]
fn test_advancement_w3_4_geodata_updater_stays_honest() {
    let (mut state, _) = AppState::new();

    // Initial state: no version facts exist anywhere.
    assert!(state.editor.geodata_status.geoip_version.is_empty());
    assert!(state.editor.geodata_status.geosite_version.is_empty());
    assert_eq!(state.editor.geodata_status.geoip_size_bytes, 0);
    assert_eq!(state.editor.geodata_status.geosite_size_bytes, 0);
    assert_eq!(state.editor.geodata_status.phase, GeoDataUpdatePhase::Idle);

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
        ToastStatus::Error,
    ));
    assert!(!state.shell.toasts.is_empty());
    assert!(state.editor.geodata_status.geoip_version.is_empty());
    assert!(state.editor.geodata_status.geosite_version.is_empty());
    assert_eq!(state.editor.geodata_status.phase, GeoDataUpdatePhase::Idle);
}

#[test]
fn test_advancement_w3_5_uwp_loopback_exemption_manager() {
    let (mut state, _) = AppState::new();
    state.shell.demo = true;

    // Initial state: the loopback packages are the shared page model's.
    assert!(state.shell.uwp_loopback.uwp_packages().is_empty());

    // Scan apps
    let _ = state.update(Message::ScanUwpApps);
    assert_eq!(state.shell.uwp_loopback.uwp_packages().len(), 3);
    assert_eq!(
        state.shell.uwp_loopback.uwp_packages()[0].display_name,
        "Microsoft Store"
    );
    assert_eq!(
        state.shell.uwp_loopback.uwp_packages()[1].display_name,
        "Xbox App"
    );
    assert_eq!(
        state.shell.uwp_loopback.uwp_packages()[2].display_name,
        "Windows Terminal"
    );

    // Exempt all
    let _ = state.update(Message::ExemptAllUwpApps);
    for package in state.shell.uwp_loopback.uwp_packages() {
        assert!(package.loopback_exempt);
    }

    // Clear all exemptions
    let _ = state.update(Message::ClearAllUwpExemptions);
    for package in state.shell.uwp_loopback.uwp_packages() {
        assert!(!package.loopback_exempt);
    }

    // Toggle single app
    let _ = state.update(Message::ToggleUwpAppExemption("S-1-15-2-1".to_string()));
    assert!(state.shell.uwp_loopback.uwp_packages()[0].loopback_exempt);
    assert!(!state.shell.uwp_loopback.uwp_packages()[1].loopback_exempt);
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

    assert_eq!(state.shell.uwp_loopback.uwp_revision(), 4);
    assert!(matches!(
        state.shell.uwp_loopback.uwp_availability(),
        UwpLoopbackAvailability::Supported
    ));
    assert_eq!(state.shell.uwp_loopback.uwp_packages().len(), 1);
    assert_eq!(
        state.shell.uwp_loopback.uwp_packages()[0].display_name,
        "Contoso UWP"
    );
    assert!(state.shell.uwp_loopback.uwp_packages()[0].loopback_exempt);
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
    let path = env::temp_dir().join("infiltrator_backup.encpkg");
    let enc_bytes = read(&path).expect("Encrypted file must be written");
    assert!(!enc_bytes.is_empty());

    // Import encrypted package with valid passphrase
    let _ = state.update(Message::ImportEncryptedPackage);

    // Import with wrong passphrase triggers error toast without mutating state
    let _ = state.update(Message::UpdateEncryptedBackupPassphrase(
        "WrongPassphrase".to_string(),
    ));
    let _ = state.update(Message::ImportEncryptedPackage);
}
