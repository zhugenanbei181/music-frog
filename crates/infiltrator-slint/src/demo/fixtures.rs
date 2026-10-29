//! The deterministic fixtures the Slint capture reads.
//!
//! Every workbench panel of the capture shows a real shared read model, so a
//! receipt proves an interaction and never an empty panel. The fixtures are
//! the shared contract's own types: the surface never invents a fact.

use infiltrator_contract::aggregator::{AggregationDraft, AggregationReport};
use infiltrator_contract::app_routing::{AppRoutingMode, AppRoutingRule};
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
use infiltrator_contract::doctor::{DoctorCheckKind, DoctorStatus};
use infiltrator_contract::script_export::{
    ScriptExportKind, ScriptExportOutcome, ScriptExportSnapshot,
};
use infiltrator_contract::script_sandbox::ScriptSandboxSnapshot;
use infiltrator_contract::snapshot_history::{SnapshotEntry, SnapshotHistorySnapshot};
use infiltrator_contract::subscription_import::SubscriptionFilterDraft;
use infiltrator_contract::surface_snapshot::{
    AppRoutingPageSnapshot, AppSnapshot, DoctorCheckSnapshot, DoctorPageSnapshot,
};
use infiltrator_contract::sync::SyncStatus;
use infiltrator_contract::sync_snapshot::{
    SnapshotItemSnapshot, SyncConflictSnapshot, SyncPageSnapshot,
};
use infiltrator_contract::yaml_ast_diff::YamlAstDiffSnapshot;
use lurker_core_contract::identity::EngineId;
use lurker_core_contract::page::PageStatus;
use lurker_core_contract::profile_document_identity::{
    ProfileDocumentIdentity, ProfileDocumentRevision, ProfileDocumentSourceIdentity,
    ProfileDocumentSourceKind, ProfileFormatId, ProfileId, ProfileSchemaVersion,
    ProfileSidecarRevision, ProfileSourceId, ProfileWorkspaceIdentity,
};

/// The DNS leak report of the capture fixture.
///
/// Two configured sources observed the same resolver identity, so the panel
/// shows a real conclusion and the real facts behind it.
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

/// The DNS cache flush report of the capture fixture.
///
/// The Fake-IP target accepted the flush; the host resolver has no drivable
/// cache, so its outcome stays typed.
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
        auto_flush_generation: None,
    }
}

/// The DNS query report of the capture fixture.
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

/// The Hosts profile of the capture fixture.
///
/// The preserved legacy rows are part of the profile, so the migration path of
/// the editor has something real to import.
pub fn dns_hosts_profile() -> DnsHostsProfile {
    DnsHostsProfile {
        profile: "profile-1".to_owned(),
        entries: vec![
            DnsHostEntry {
                domain: "ads.example.com".to_owned(),
                address: "0.0.0.0".to_owned(),
            },
            DnsHostEntry {
                domain: "cdn.example.com".to_owned(),
                address: "203.0.113.10".to_owned(),
            },
        ],
        legacy_entries: vec![DnsHostEntry {
            domain: "legacy.example.com".to_owned(),
            address: "127.0.0.1".to_owned(),
        }],
    }
}

/// The diagnostic report of the capture fixture.
pub fn doctor_page() -> DoctorPageSnapshot {
    DoctorPageSnapshot {
        overall_healthy: false,
        last_run: "2026-10-20 12:00:00".to_owned(),
        checks: vec![
            DoctorCheckSnapshot {
                kind: Some(DoctorCheckKind::Configuration),
                detail_copy_key: None,
                id: "capture.configuration".to_owned(),
                name: "Configuration".to_owned(),
                category: "config".to_owned(),
                state: DoctorStatus::Fail,
                detail: "the active profile failed the shared validation".to_owned(),
                fix_available: true,
                hint: None,
            },
            DoctorCheckSnapshot {
                kind: Some(DoctorCheckKind::DnsPrivacy),
                detail_copy_key: None,
                id: "capture.dns-privacy".to_owned(),
                name: "DNS privacy".to_owned(),
                category: "dns".to_owned(),
                state: DoctorStatus::Warn,
                detail: "the system resolver still answers the fallback zone".to_owned(),
                fix_available: false,
                hint: None,
            },
        ],
        report_started_at: Some(1),
        report_finished_at: Some(2),
    }
}

/// The synchronisation page of the capture fixture.
///
/// The remote diverged, so the conflict resolver and the stored snapshots are
/// both real.
pub fn sync_page() -> SyncPageSnapshot {
    SyncPageSnapshot {
        status: SyncStatus::Conflict,
        server_url: "https://dav.example.org".to_owned(),
        username: "alice".to_owned(),
        last_sync: Some("2026-10-20 11:40:00".to_owned()),
        auto_sync: true,
        conflict: Some(SyncConflictSnapshot {
            remote_device: "Pixel 9 Pro".to_owned(),
            conflict_time: "2026-10-20 11:38:00".to_owned(),
            conflicting_keys: vec![
                (
                    "mixed-port".to_owned(),
                    "7890".to_owned(),
                    "7891".to_owned(),
                ),
                ("mode".to_owned(), "rule".to_owned(), "global".to_owned()),
            ],
        }),
        snapshots: vec![
            SnapshotItemSnapshot {
                profile: "profile-1".to_owned(),
                id: "snap-20261020".to_owned(),
                timestamp: "2026-10-20 10:05:00".to_owned(),
                device: "desktop".to_owned(),
                size_bytes: Some(48 * 1024),
            },
            SnapshotItemSnapshot {
                profile: "profile-1".to_owned(),
                id: "snap-20261019".to_owned(),
                timestamp: "2026-10-19 22:15:00".to_owned(),
                device: "Pixel 9 Pro".to_owned(),
                size_bytes: Some(46 * 1024),
            },
        ],
        history_status: PageStatus::Ready,
    }
}

/// The application routing policy of the capture fixture.
pub fn app_routing_page() -> AppRoutingPageSnapshot {
    AppRoutingPageSnapshot {
        mode: AppRoutingMode::ProxySelected,
        include_system: false,
        apps: vec![
            AppSnapshot {
                id: "com.example.browser".to_owned(),
                name: "Example Browser".to_owned(),
                process_name: "example-browser".to_owned(),
                rule: AppRoutingRule::Proxy,
                is_system: false,
            },
            AppSnapshot {
                id: "com.example.chat".to_owned(),
                name: "Example Chat".to_owned(),
                process_name: "example-chat".to_owned(),
                rule: AppRoutingRule::Direct,
                is_system: false,
            },
            AppSnapshot {
                id: "com.example.updater".to_owned(),
                name: "Example Updater".to_owned(),
                process_name: "example-updater".to_owned(),
                rule: AppRoutingRule::Block,
                is_system: false,
            },
        ],
        uwp_loopback: Default::default(),
    }
}

/// The script sandbox report of the capture fixture.
pub fn script_sandbox() -> ScriptSandboxSnapshot {
    ScriptSandboxSnapshot::demo_fixture()
}

/// The reviewed script export of the capture fixture.
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

/// The configuration difference of the capture fixture.
pub fn yaml_ast_diff() -> YamlAstDiffSnapshot {
    YamlAstDiffSnapshot::demo_fixture()
}

/// The stored snapshots of the capture fixture.
pub fn snapshot_history() -> SnapshotHistorySnapshot {
    SnapshotHistorySnapshot {
        profile: "profile-1".to_owned(),
        entries: vec![
            SnapshotEntry {
                id: "snap-20261020".to_owned(),
                file_name: "profile-1-20261020.yaml".to_owned(),
                timestamp_millis: 1_767_225_600_000,
                sha256: "b".repeat(64),
                is_newest: true,
                is_duplicate: false,
            },
            SnapshotEntry {
                id: "snap-20261019".to_owned(),
                file_name: "profile-1-20261019.yaml".to_owned(),
                timestamp_millis: 1_758_566_400_000,
                sha256: "a".repeat(64),
                is_newest: false,
                is_duplicate: false,
            },
        ],
        keep_limit: 5,
        pending_prune: 0,
        duplicate_entries: 0,
        last_prune: None,
    }
}

/// The aggregation report of the capture fixture.
pub fn aggregation_report() -> AggregationReport {
    AggregationReport {
        draft: AggregationDraft {
            source_profiles: vec!["profile-1".to_owned(), "profile-2".to_owned()],
            target_name: "Merged".to_owned(),
            deduplicate: true,
            deduplicate_names: true,
            geo_cluster: true,
            generate_groups: true,
            availability_precheck: true,
            ..Default::default()
        },
        source_count: 2,
        missing_sources: Vec::new(),
        input_nodes: 24,
        total_nodes: 21,
        duplicates_removed: 2,
        renamed_nodes: 1,
        rule_renamed_nodes: 0,
        invalid_nodes_removed: 1,
        invalid_node_samples: vec!["Example Node (missing port)".to_owned()],
        regions: Vec::new(),
        groups: Vec::new(),
        yaml: "proxies:\n  - name: HK-01\n    server: hk-01.node.example.com\n".to_owned(),
        generated_at: "2026-10-20T12:00:00Z".to_owned(),
    }
}

/// The workspace identity of the captured profile document.
///
/// The filter editor reads its source from the owner snapshot, so the capture
/// publishes a real document identity instead of an empty source.
pub fn profile_workspace_identity() -> ProfileWorkspaceIdentity {
    let document = ProfileDocumentIdentity::new(
        ProfileId::new("profile-1").expect("valid profile id"),
        EngineId::new("mihomo").expect("valid engine id"),
        ProfileDocumentSourceIdentity::new(
            ProfileDocumentSourceKind::Local,
            ProfileSourceId::new("demo-fixture").expect("valid source id"),
        ),
        ProfileDocumentRevision::new("demo-revision").expect("valid revision"),
        ProfileFormatId::new("mihomo-yaml").expect("valid format id"),
        ProfileSchemaVersion::new(1).expect("valid schema version"),
    );
    ProfileWorkspaceIdentity::new(document, ProfileSidecarRevision::Absent)
}

/// The subscription filter draft of the captured profile.
pub fn subscription_filter() -> SubscriptionFilterDraft {
    SubscriptionFilterDraft {
        include: "香港|日本".to_owned(),
        exclude: "剩余|到期".to_owned(),
        exclude_types: String::new(),
        renames: "旧名字 => 新名字".to_owned(),
        dedup_index: 0,
        advanced_policy: None,
    }
}
