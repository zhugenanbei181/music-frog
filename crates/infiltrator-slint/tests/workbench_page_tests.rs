//! SLINT-001.2 .. SLINT-001.7: the workbench pages of the Slint surface.
//!
//! Every case drives the production Slint callbacks through the headless
//! surface harness. The dispatched commands are the shared `CommandIntent`
//! vocabulary and the rendered cells are the shared presentation reductions,
//! so a surface that drifted from the shared contract fails here.

use infiltrator_contract::app_routing::{AppRoutingMode, AppRoutingRule};
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::dns_hosts::{DnsHostEntry, DnsHostsProfile};
use infiltrator_contract::snapshot_history::{SnapshotEntry, SnapshotHistorySnapshot};
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::{
    AppRoutingPageSnapshot, DoctorPageSnapshot, ProfilesPageSnapshot, SurfaceSnapshot,
};
use infiltrator_contract::sync_snapshot::SyncPageSnapshot;
use infiltrator_slint::SlintAction;
use infiltrator_slint::surface::action::workbench;
use infiltrator_slint::surface::harness;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::EngineId;
use lurker_core_contract::page::PageData;
use lurker_core_contract::profile_document_identity::{
    ProfileDocumentIdentity, ProfileDocumentRevision, ProfileDocumentSourceIdentity,
    ProfileDocumentSourceKind, ProfileFormatId, ProfileId, ProfileSchemaVersion,
    ProfileSidecarRevision, ProfileSourceId, ProfileWorkspaceIdentity,
};
use serde_json::json;

/// The typed identity of the fixture profile document.
fn workspace_identity(profile: &str) -> ProfileWorkspaceIdentity {
    let document = ProfileDocumentIdentity::new(
        ProfileId::new(profile).expect("valid profile id"),
        EngineId::new("mihomo").expect("valid engine id"),
        ProfileDocumentSourceIdentity::new(
            ProfileDocumentSourceKind::Local,
            ProfileSourceId::new("local-fixture").expect("valid source id"),
        ),
        ProfileDocumentRevision::new("fixture-revision").expect("valid revision"),
        ProfileFormatId::new("mihomo-yaml").expect("valid format id"),
        ProfileSchemaVersion::new(1).expect("valid schema version"),
    );
    ProfileWorkspaceIdentity::new(document, ProfileSidecarRevision::Absent)
}

/// The owner snapshot the workbench pages observe.
fn workbench_snapshot() -> SurfaceSnapshot {
    let mut snapshot = SurfaceSnapshot::unavailable(
        SurfaceKind::SlintDesktop,
        HostKind::Desktop,
        Failure::new(ErrorCode::NotReady, "initial", true),
    );

    let mut profiles: ProfilesPageSnapshot = serde_json::from_value(json!({
        "profiles": [{
            "id": "main",
            "name": "Main",
            "url": "https://example.org/main.yaml",
            "updated_at": "2026-01-01T00:00:00Z",
            "upload_bytes": 0,
            "download_bytes": 0,
            "total_bytes": 0,
            "is_active": true
        }],
        "auto_update_interval_hours": 12,
        "updating": false
    }))
    .expect("profiles page JSON");
    profiles.profiles[0].filter_source = Ok(workspace_identity("main"));
    snapshot.pages.profiles = PageData::ready(profiles);
    snapshot
        .pages
        .profiles
        .data
        .as_mut()
        .expect("profiles page")
        .snapshot_history = Some(SnapshotHistorySnapshot {
        profile: "main".to_owned(),
        entries: vec![SnapshotEntry {
            id: "snap-1".to_owned(),
            file_name: "snap-1.yaml".to_owned(),
            timestamp_millis: 0,
            sha256: "a".repeat(64),
            is_newest: true,
            is_duplicate: false,
        }],
        keep_limit: 5,
        pending_prune: 0,
        duplicate_entries: 0,
        last_prune: None,
    });

    let doctor: DoctorPageSnapshot = serde_json::from_value(json!({
        "overall_healthy": false,
        "last_run": "2026-01-01T00:00:00Z",
        "checks": [{
            "id": "capture.configuration",
            "name": "Configuration",
            "category": "config",
            "state": "fail",
            "detail": "configuration is invalid",
            "fix_available": true
        }],
        "report_started_at": 1,
        "report_finished_at": 2
    }))
    .expect("doctor page JSON");
    snapshot.pages.doctor = PageData::ready(doctor);

    let sync: SyncPageSnapshot = serde_json::from_value(json!({
        "status": "conflict",
        "server_url": "https://dav.example.org",
        "username": "alice",
        "last_sync": "2026-01-01T00:00:00Z",
        "auto_sync": true,
        "conflict": {
            "remote_device": "phone",
            "conflict_time": "2026-01-02T00:00:00Z",
            "conflicting_keys": [["port", "7890", "7891"]]
        },
        "snapshots": [{
            "profile": "main",
            "id": "snap-1",
            "timestamp": "2026-01-01T00:00:00Z",
            "device": "phone",
            "size_bytes": 128
        }]
    }))
    .expect("sync page JSON");
    snapshot.pages.sync = PageData::ready(sync);

    let routing: AppRoutingPageSnapshot = serde_json::from_value(json!({
        "mode": "proxy_selected",
        "include_system": false,
        "apps": [{
            "id": "com.example.app",
            "name": "Example",
            "process_name": "example",
            "rule": "direct",
            "is_system": false
        }]
    }))
    .expect("app routing page JSON");
    snapshot.pages.app_routing = PageData::ready(routing);

    snapshot.dns_hosts = PageData::ready(DnsHostsProfile {
        profile: "main".to_owned(),
        entries: vec![DnsHostEntry {
            domain: "live.test".to_owned(),
            address: "9.9.9.9".to_owned(),
        }],
        legacy_entries: vec![DnsHostEntry {
            domain: "legacy.test".to_owned(),
            address: "1.1.1.1".to_owned(),
        }],
    });
    snapshot
}

/// Asserts that the surface dispatched exactly one owner command.
fn single_command(outcome: harness::SurfaceOutcome) -> CommandIntent {
    match outcome {
        harness::SurfaceOutcome::Dispatched(actions) => {
            assert_eq!(
                actions.len(),
                1,
                "expected one owner action, got {actions:?}"
            );
            match &actions[0] {
                SlintAction::Command(intent) => intent.clone(),
                other => panic!("expected an owner command, got {other:?}"),
            }
        }
        harness::SurfaceOutcome::Rejected { failure, .. } => {
            panic!("expected an owner command, got the failure {failure:?}")
        }
    }
}

/// Asserts that the surface refused the gesture with the given typed failure.
fn rejection(outcome: harness::SurfaceOutcome) -> Failure {
    match outcome {
        harness::SurfaceOutcome::Dispatched(actions) => {
            panic!("expected a refusal, got the owner actions {actions:?}")
        }
        harness::SurfaceOutcome::Rejected { failure, actions } => {
            assert!(
                actions.is_empty(),
                "a refused gesture must not dispatch owner actions, got {actions:?}"
            );
            failure
        }
    }
}

/// Opens one workbench page on a surface that observes the fixture snapshot.
fn workbench_surface(page: &str) -> harness::SurfaceHarness {
    let snapshot = workbench_snapshot();
    let surface = harness::SurfaceHarness::new(Some(snapshot.clone())).expect("headless surface");
    surface.open_page(page);
    surface
}

#[test]
fn the_dns_leak_alert_dispatches_the_shared_probe() {
    let surface = workbench_surface("Dns");

    let command = single_command(surface.workbench_action(workbench::DNS_LEAK_PROBE, ""));

    assert_eq!(command, CommandIntent::TestDnsLeak);
}

#[test]
fn the_dns_leak_alert_projects_the_shared_conclusion() {
    let surface = workbench_surface("Dns");

    let item = surface
        .workbench_item("dns-leak")
        .expect("the leak panel is projected");

    assert_eq!(item.kind.as_str(), "header");
    assert!(!item.status.is_empty());
}

#[test]
fn the_dns_query_panel_dispatches_the_shared_query_command() {
    let surface = workbench_surface("Dns");
    surface.set_workbench_draft("example.org");

    let command = single_command(surface.workbench_action(workbench::DNS_QUERY_RUN, ""));

    let CommandIntent::QueryDns { request, .. } = command else {
        panic!("expected the shared DNS query command");
    };
    assert_eq!(request.name, "example.org");
}

#[test]
fn the_dns_query_panel_refuses_an_invalid_name() {
    let surface = workbench_surface("Dns");
    surface.set_workbench_draft("not a name");

    let failure = rejection(surface.workbench_action(workbench::DNS_QUERY_RUN, ""));

    assert_eq!(failure.code, ErrorCode::InvalidInput);
    assert_eq!(surface.notice(), failure.message);
}

#[test]
fn the_dns_cache_flush_dispatches_the_shared_flush_command() {
    let surface = workbench_surface("Dns");

    let command = single_command(surface.workbench_action(workbench::DNS_CACHE_CONFIRM, ""));

    assert!(matches!(command, CommandIntent::ClearDnsCache { .. }));
}

#[test]
fn the_dns_cache_flush_can_be_cancelled_without_the_owner() {
    let surface = workbench_surface("Dns");

    match surface.workbench_action(workbench::DNS_CACHE_CANCEL, "") {
        harness::SurfaceOutcome::Dispatched(actions) => assert!(
            actions.is_empty(),
            "cancelling the flush must not ask the owner for anything, got {actions:?}"
        ),
        harness::SurfaceOutcome::Rejected { failure, .. } => {
            panic!("expected the cancellation to succeed, got the failure {failure:?}")
        }
    }
}

#[test]
fn the_dns_hosts_editor_commits_the_shared_guarded_write() {
    let surface = workbench_surface("Dns");
    surface.set_workbench_draft_2("8.8.8.8 new.test");
    match surface.workbench_action(workbench::DNS_HOSTS_ADD, "") {
        harness::SurfaceOutcome::Dispatched(_) => {}
        harness::SurfaceOutcome::Rejected { failure, .. } => {
            panic!("expected the row to be drafted, got the failure {failure:?}")
        }
    }

    let command = single_command(surface.workbench_action(workbench::DNS_HOSTS_SAVE, ""));

    let CommandIntent::ApplyDnsSettings { patch } = command else {
        panic!("expected the shared DNS settings patch");
    };
    assert_eq!(patch.expected_profile.as_deref(), Some("main"));
    assert!(
        patch
            .hosts
            .as_ref()
            .is_some_and(|entries| entries.iter().any(|entry| entry.domain == "new.test"))
    );
}

#[test]
fn the_dns_hosts_editor_refuses_an_invalid_row() {
    let surface = workbench_surface("Dns");
    surface.set_workbench_draft_2("not an address new.test");

    let failure = rejection(surface.workbench_action(workbench::DNS_HOSTS_ADD, ""));

    assert_eq!(failure.code, ErrorCode::InvalidInput);
}

#[test]
fn the_script_sandbox_dispatches_the_shared_run_command() {
    let surface = workbench_surface("Editor");
    surface.set_workbench_draft("preset('remove-ads')");

    let command = single_command(surface.workbench_action(workbench::SCRIPT_RUN, ""));

    let CommandIntent::RunScriptSandbox { request } = command else {
        panic!("expected the shared script sandbox command");
    };
    assert_eq!(request.script_code, "preset('remove-ads')");
}

#[test]
fn the_script_sandbox_clears_through_the_shared_command() {
    let surface = workbench_surface("Editor");

    let command = single_command(surface.workbench_action(workbench::SCRIPT_CLEAR, ""));

    assert!(matches!(command, CommandIntent::ClearScriptSandbox { .. }));
}

#[test]
fn the_script_export_review_prepares_the_shared_artifact() {
    let surface = workbench_surface("Editor");

    let command = single_command(surface.workbench_action(workbench::SCRIPT_EXPORT_PREPARE, ""));

    let CommandIntent::PrepareScriptExport { draft } = command else {
        panic!("expected the shared script export command");
    };
    assert_eq!(draft.script_code, "");
}

#[test]
fn the_script_export_review_refuses_a_second_prepare() {
    let surface = workbench_surface("Editor");
    match surface.workbench_action(workbench::SCRIPT_EXPORT_PREPARE, "") {
        harness::SurfaceOutcome::Dispatched(_) => {}
        harness::SurfaceOutcome::Rejected { failure, .. } => {
            panic!("expected the preparation to run, got the failure {failure:?}")
        }
    }

    let failure = rejection(surface.workbench_action(workbench::SCRIPT_EXPORT_PREPARE, ""));

    assert_eq!(failure.code, ErrorCode::InvalidState);
}

#[test]
fn the_configuration_difference_loads_the_shared_diff() {
    let surface = workbench_surface("Editor");

    let command = single_command(surface.workbench_action(workbench::DIFF_LOAD, ""));

    assert_eq!(
        command,
        CommandIntent::LoadSnapshotDiff {
            profile: None,
            snapshot_id: None
        }
    );
}

#[test]
fn the_snapshot_restoration_prepares_the_shared_review() {
    let surface = workbench_surface("Editor");

    let command = single_command(surface.workbench_action(workbench::RESTORE_PREPARE, ""));

    let CommandIntent::PrepareSnapshotRestore { target } = command else {
        panic!("expected the shared snapshot restoration command");
    };
    assert_eq!(target.profile, "main");
    assert_eq!(target.snapshot_id, "snap-1");
}

#[test]
fn the_subscription_filter_commits_the_shared_pipeline() {
    let surface = workbench_surface("Filters");
    match surface.workbench_draft_edited("filter-include", "hk|jp") {
        harness::SurfaceOutcome::Dispatched(_) => {}
        harness::SurfaceOutcome::Rejected { failure, .. } => {
            panic!("expected the draft to be accepted, got the failure {failure:?}")
        }
    }

    let command = single_command(surface.workbench_action(workbench::FILTER_SAVE, ""));

    let CommandIntent::SaveSubscriptionFilter {
        workspace_identity,
        filter,
    } = command
    else {
        panic!("expected the shared subscription filter command");
    };
    assert_eq!(workspace_identity.document.profile_id.as_str(), "main");
    assert_eq!(filter.include, "hk|jp");
}

#[test]
fn the_profile_aggregator_previews_the_shared_report() {
    let surface = workbench_surface("Filters");
    surface.set_workbench_draft_2("Merged");

    let command = single_command(surface.workbench_action(workbench::AGGREGATE_PREVIEW, ""));

    let CommandIntent::PreviewProfileAggregation { draft } = command else {
        panic!("expected the shared aggregation preview command");
    };
    assert_eq!(draft.target_name, "Merged");
    assert_eq!(draft.source_profiles, vec!["main".to_owned()]);
}

#[test]
fn the_profile_aggregator_creates_the_shared_profile() {
    let surface = workbench_surface("Filters");
    surface.set_workbench_draft_2("Merged");

    let command = single_command(surface.workbench_action(workbench::AGGREGATE_CREATE, ""));

    assert!(matches!(
        command,
        CommandIntent::CreateAggregatedProfile { .. }
    ));
}

#[test]
fn the_profile_aggregator_refuses_an_empty_target() {
    let surface = workbench_surface("Filters");
    surface.set_workbench_draft_2("  ");

    let failure = rejection(surface.workbench_action(workbench::AGGREGATE_PREVIEW, ""));

    assert_eq!(failure.code, ErrorCode::InvalidInput);
}

#[test]
fn the_diagnostics_run_the_shared_report() {
    let surface = workbench_surface("Doctor");

    let command = single_command(surface.workbench_action(workbench::DOCTOR_DIAGNOSE, ""));

    assert_eq!(command, CommandIntent::RunDoctorDiagnostics);
}

#[test]
fn the_diagnostics_repair_one_shared_issue() {
    let surface = workbench_surface("Doctor");

    let command =
        single_command(surface.workbench_action(workbench::DOCTOR_REPAIR, "capture.configuration"));

    assert_eq!(
        command,
        CommandIntent::RepairDoctorIssue {
            check_id: "capture.configuration".to_owned()
        }
    );
}

#[test]
fn the_diagnostics_repair_every_shared_issue() {
    let surface = workbench_surface("Doctor");

    let command = single_command(surface.workbench_action(workbench::DOCTOR_REPAIR_ALL, ""));

    assert_eq!(command, CommandIntent::RepairAllDoctorIssues);
}

#[test]
fn the_diagnostics_bootstrap_the_shared_report() {
    let surface = workbench_surface("Doctor");

    let command = single_command(surface.workbench_action(workbench::DOCTOR_BOOTSTRAP, ""));

    assert_eq!(command, CommandIntent::BootstrapDoctor);
}

#[test]
fn the_diagnostics_project_the_shared_report() {
    let surface = workbench_surface("Doctor");

    let item = surface
        .workbench_item("doctor-report")
        .expect("the diagnostic panel is projected");

    assert_eq!(item.kind.as_str(), "header");
    assert!(!item.status.is_empty());
}

#[test]
fn the_sync_page_synchronises_through_the_shared_command() {
    let surface = workbench_surface("Sync");

    let command = single_command(surface.workbench_action(workbench::SYNC_NOW, ""));

    assert_eq!(command, CommandIntent::SyncNow);
}

#[test]
fn the_sync_conflict_keeps_the_local_copy() {
    let surface = workbench_surface("Sync");

    let command = single_command(surface.workbench_action(workbench::SYNC_KEEP_LOCAL, ""));

    assert_eq!(command, CommandIntent::ResolveConflictKeepLocal);
}

#[test]
fn the_sync_conflict_takes_the_remote_copy() {
    let surface = workbench_surface("Sync");

    let command = single_command(surface.workbench_action(workbench::SYNC_TAKE_REMOTE, ""));

    assert_eq!(command, CommandIntent::ResolveConflictTakeRemote);
}

#[test]
fn the_sync_page_creates_the_shared_backup() {
    let surface = workbench_surface("Sync");

    let command = single_command(surface.workbench_action(workbench::SYNC_BACKUP, ""));

    assert_eq!(
        command,
        CommandIntent::CreateBackupSnapshot { profile: None }
    );
}

#[test]
fn the_sync_page_projects_the_shared_conflict() {
    let surface = workbench_surface("Sync");

    let item = surface
        .workbench_item("sync-conflict")
        .expect("the conflict panel is projected");

    assert_eq!(item.kind.as_str(), "header");
    assert!(!item.status.is_empty());
}

#[test]
fn the_app_routing_policy_changes_the_shared_mode() {
    let surface = workbench_surface("AppRouting");

    let command = single_command(surface.workbench_action(workbench::APP_ROUTING_MODE_ALL, ""));

    assert_eq!(
        command,
        CommandIntent::SetAppRoutingMode {
            mode: AppRoutingMode::ProxyAll
        }
    );
}

#[test]
fn the_app_routing_policy_changes_the_shared_rule() {
    let surface = workbench_surface("AppRouting");
    surface.set_workbench_draft_2("block");

    let command =
        single_command(surface.workbench_action(workbench::APP_ROUTING_RULE, "com.example.app"));

    assert_eq!(
        command,
        CommandIntent::SetAppRule {
            app_id: "com.example.app".to_owned(),
            rule: AppRoutingRule::Block
        }
    );
}

#[test]
fn the_app_routing_policy_refuses_an_unknown_rule() {
    let surface = workbench_surface("AppRouting");
    surface.set_workbench_draft_2("nonsense");

    let failure =
        rejection(surface.workbench_action(workbench::APP_ROUTING_RULE, "com.example.app"));

    assert_eq!(failure.code, ErrorCode::InvalidInput);
}

#[test]
fn the_app_routing_page_projects_the_shared_policy() {
    let surface = workbench_surface("AppRouting");

    let item = surface
        .workbench_item("app-routing")
        .expect("the routing panel is projected");

    assert_eq!(item.kind.as_str(), "header");
    assert!(!item.status.is_empty());
}

#[test]
fn an_unknown_workbench_action_is_refused_with_the_typed_failure() {
    let surface = workbench_surface("Dns");

    let failure = rejection(surface.workbench_action("no-such-action", ""));

    assert_eq!(failure.code, ErrorCode::InvalidInput);
    assert_eq!(failure.message, "Unknown workbench action");
}
