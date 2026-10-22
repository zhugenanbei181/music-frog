//! The shared fixture of the Slint workbench path tests.
//!
//! Every panel of the fixture observes the shared read models of the
//! businesses it carries, so a driven test sees the same panel a capture
//! photographs.

use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::dns_cache::{
    DnsCacheFlushReport, DnsCacheOperation, DnsCacheSnapshot, DnsFlushOutcome,
};
use infiltrator_contract::dns_hosts::{DnsHostEntry, DnsHostsProfile};
use infiltrator_contract::dns_leak::{
    DnsLeakEchoRecord, DnsLeakObservation, DnsLeakProbeSource, DnsLeakProbeTransport, DnsLeakReport,
};
use infiltrator_contract::dns_query::{
    DnsQueryOperation, DnsQueryReport, DnsQueryRequest, DnsQueryResponse, DnsQuerySnapshot,
    DnsQuestion, DnsRecord, DnsRecordType,
};
use infiltrator_contract::language::LanguagePreference;
use infiltrator_contract::script_export::{
    ScriptExportKind, ScriptExportOutcome, ScriptExportSnapshot,
};
use infiltrator_contract::snapshot_history::{SnapshotEntry, SnapshotHistorySnapshot};
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::{
    AppRoutingPageSnapshot, DoctorPageSnapshot, ProfilesPageSnapshot, SurfaceSnapshot,
};
use infiltrator_contract::sync_snapshot::SyncPageSnapshot;
use infiltrator_slint::SlintAction;
use infiltrator_slint::surface::harness::SurfaceHarness;
use infiltrator_slint::surface::harness::SurfaceOutcome;
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
pub fn workspace_identity(profile: &str) -> ProfileWorkspaceIdentity {
    workspace_identity_with_revision(profile, "fixture-revision")
}

/// The typed identity of a fixture profile document at one revision.
pub fn workspace_identity_with_revision(profile: &str, revision: &str) -> ProfileWorkspaceIdentity {
    let document = ProfileDocumentIdentity::new(
        ProfileId::new(profile).expect("valid profile id"),
        EngineId::new("mihomo").expect("valid engine id"),
        ProfileDocumentSourceIdentity::new(
            ProfileDocumentSourceKind::Local,
            ProfileSourceId::new("local-fixture").expect("valid source id"),
        ),
        ProfileDocumentRevision::new(revision).expect("valid revision"),
        ProfileFormatId::new("mihomo-yaml").expect("valid format id"),
        ProfileSchemaVersion::new(1).expect("valid schema version"),
    );
    ProfileWorkspaceIdentity::new(document, ProfileSidecarRevision::Absent)
}

/// The owner snapshot the workbench panels observe.
pub fn workbench_snapshot() -> SurfaceSnapshot {
    let mut snapshot = SurfaceSnapshot::unavailable(
        SurfaceKind::SlintDesktop,
        HostKind::Desktop,
        Failure::new(ErrorCode::NotReady, "initial", true),
    );
    snapshot.language_settings.preference = Some(LanguagePreference::English);

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
    profiles.aggregation = Some(
        serde_json::from_value(json!({
            "source_count": 2,
            "input_nodes": 24,
            "total_nodes": 21,
            "duplicates_removed": 2,
            "generated_at": "2026-01-01T00:00:00Z",
            "draft": {
                "source_profiles": ["main"],
                "target_name": "Merged"
            }
        }))
        .expect("aggregation report JSON"),
    );
    snapshot.pages.profiles = PageData::ready(profiles);
    snapshot
        .pages
        .profiles
        .data
        .as_mut()
        .expect("profiles page")
        .snapshot_history = Some(snapshot_history());

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
    snapshot.dns_leak = dns_leak_report();
    snapshot.dns_cache = dns_cache_snapshot();
    snapshot.dns_query = dns_query_snapshot();
    snapshot.script_export = Some(script_export());
    snapshot
}

/// The stored snapshots of the fixture profile.
pub fn snapshot_history() -> SnapshotHistorySnapshot {
    SnapshotHistorySnapshot {
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
    }
}

/// The DNS leak report of the fixture.
pub fn dns_leak_report() -> DnsLeakReport {
    let record = DnsLeakEchoRecord::TxtKeyedValue {
        key: "id".to_owned(),
    };
    let sources = vec![
        DnsLeakProbeSource::with_record("1.1.1.1:53", "echo.example.com", record.clone()),
        DnsLeakProbeSource::with_record("8.8.8.8:53", "echo.example.com", record),
    ];
    let observations = vec![
        DnsLeakObservation::observed(
            "1.1.1.1:53",
            "echo.example.com",
            "a1b2c3.echo.example.com",
            DnsLeakProbeTransport::Udp,
            "198.51.100.7",
        ),
        DnsLeakObservation::observed(
            "8.8.8.8:53",
            "echo.example.com",
            "d4e5f6.echo.example.com",
            DnsLeakProbeTransport::Doh,
            "198.51.100.7",
        ),
    ];
    DnsLeakReport::observed(sources, observations)
}

/// The DNS cache flush report of the fixture.
pub fn dns_cache_snapshot() -> DnsCacheSnapshot {
    DnsCacheSnapshot {
        revision: 1,
        operation_id: None,
        report_id: None,
        operation: DnsCacheOperation::Completed,
        report: DnsCacheFlushReport {
            fake_ip: DnsFlushOutcome::Flushed,
            os_cache: DnsFlushOutcome::Unsupported {
                reason: "the host resolver cache has no drivable flush".to_owned(),
            },
        },
        failure: None,
    }
}

/// The DNS query report of the fixture.
pub fn dns_query_snapshot() -> DnsQuerySnapshot {
    let request = DnsQueryRequest {
        name: "example.org".to_owned(),
        record_type: DnsRecordType::A,
    };
    let response = DnsQueryResponse {
        status: 0,
        questions: vec![DnsQuestion {
            name: "example.org".to_owned(),
            record_type: 1,
            class: 1,
        }],
        truncated: false,
        recursion_desired: true,
        recursion_available: true,
        authenticated_data: false,
        checking_disabled: false,
        answers: vec![DnsRecord {
            name: "example.org".to_owned(),
            record_type: 1,
            ttl: 300,
            data: "93.184.216.34".to_owned(),
        }],
        authority: Vec::new(),
        additional: Vec::new(),
    };
    DnsQuerySnapshot {
        revision: 1,
        operation_id: None,
        report_id: None,
        operation: DnsQueryOperation::Completed,
        report: Some(DnsQueryReport { request, response }),
        failure: None,
    }
}

/// The typed refusal the owner answers a command with.
///
/// The refusal is a permission failure, so the shared reducers offer the
/// retry the product offers.
pub fn refusal(message: &str) -> Failure {
    Failure::new(ErrorCode::Permission, message, true)
}

/// The typed refusal of a host that cannot offer the capability.
pub fn unsupported(message: &str) -> Failure {
    Failure::unsupported(message)
}

/// The reviewed script export the owner publishes.
pub fn script_export() -> ScriptExportSnapshot {
    ScriptExportSnapshot {
        kind: ScriptExportKind::DirectiveDslScript,
        profile: None,
        file_name: "profile-script.js".to_owned(),
        media_type: "text/javascript".to_owned(),
        content: "function main(config, profile) {\n    return config;\n}\n".to_owned(),
        checksum: None,
        honest_note: "the artifact was composed and is ready for the review".to_owned(),
        outcome: ScriptExportOutcome::Prepared,
    }
}

/// Opens one workbench page on a surface that observes the fixture snapshot.
pub fn workbench_surface(page: &str) -> SurfaceHarness {
    let snapshot = workbench_snapshot();
    let surface = SurfaceHarness::new(Some(snapshot)).expect("headless surface");
    surface.open_page(page);
    surface
}

/// Asserts that the surface dispatched exactly one owner command.
pub fn single_command(outcome: SurfaceOutcome) -> CommandIntent {
    match outcome {
        SurfaceOutcome::Dispatched(actions) => {
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
        SurfaceOutcome::Rejected { failure, .. } => {
            panic!("expected an owner command, got the failure {failure:?}")
        }
    }
}

/// Asserts that the surface refused the gesture with the given typed failure.
pub fn rejection(outcome: SurfaceOutcome) -> Failure {
    match outcome {
        SurfaceOutcome::Dispatched(actions) => {
            panic!("expected a refusal, got the owner actions {actions:?}")
        }
        SurfaceOutcome::Rejected { failure, actions } => {
            assert!(
                actions.is_empty(),
                "a refused gesture must not dispatch owner actions, got {actions:?}"
            );
            failure
        }
    }
}

/// Asserts that the surface handled the gesture without asking the owner.
pub fn local(outcome: SurfaceOutcome) {
    match outcome {
        SurfaceOutcome::Dispatched(actions) => assert!(
            actions.is_empty(),
            "a local gesture must not ask the owner for anything, got {actions:?}"
        ),
        SurfaceOutcome::Rejected { failure, .. } => {
            panic!("expected the gesture to succeed, got the failure {failure:?}")
        }
    }
}

/// Publishes the owner's refusal and the read model it publishes next.
///
/// The owner publishes its read models after every command, so the panel
/// shows the failure exactly as the product shows it.
pub fn refuse(surface: &SurfaceHarness, failure: &Failure) {
    // The owner publishes its state and then the refusal: a snapshot carries
    // the owner's facts, and the refusal is the terminal of the command.
    surface.publish_snapshot(workbench_snapshot());
    surface.publish_owner_failure(failure);
}
