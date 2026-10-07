use crate::dns_health_projection::project_dns_health;
use crate::dns_latency_projection::project_dns_latency;
use crate::dns_mapping_projection::project_mappings;
use crate::dns_observation_projection::DnsObservationTone;
use infiltrator_contract::dns::{FakeIpMappingEntry, FakeIpMappingPool, FakeIpMappingSource};
use infiltrator_contract::dns_latency::{
    DnsLatencyReport, DnsProbeOutcome, DnsProbeTransport, DnsServerLatency,
};
use infiltrator_contract::dns_self_heal::{
    DnsSelfHealCheck, DnsSelfHealFix, DnsSelfHealKind, DnsSelfHealSnapshot, DnsSelfHealState,
};

fn server(outcome: DnsProbeOutcome, fallback: bool) -> DnsServerLatency {
    DnsServerLatency {
        address: "resolver{ms}.example".into(),
        is_fallback: fallback,
        transport: DnsProbeTransport::Udp,
        outcome,
    }
}
#[test]
fn latency_summary_interpolates_all_observed_numbers_and_never_colors_unprobed_as_success() {
    for code in ["en-US", "zh-CN"] {
        let all = DnsLatencyReport::measured(
            "example.org",
            vec![
                server(DnsProbeOutcome::Measured { rtt_ms: 0 }, false),
                server(DnsProbeOutcome::Measured { rtt_ms: 100 }, true),
                server(DnsProbeOutcome::Measured { rtt_ms: 250 }, true),
            ],
        );
        let display = project_dns_latency(&all, code);
        assert!(display.summary.contains("3"));
        assert!(display.summary.contains("0-250 ms"));
        assert!(!display.summary.contains("{count}"));
        assert_eq!(display.tone, DnsObservationTone::Success);
        assert_eq!(
            display.rows.iter().map(|row| row.tone).collect::<Vec<_>>(),
            vec![
                DnsObservationTone::Success,
                DnsObservationTone::Warning,
                DnsObservationTone::Danger
            ]
        );
        assert_eq!(display.rows[0].outcome, "0 ms");
        assert_eq!(display.rows[0].address, "resolver{ms}.example");
        let timeout = DnsLatencyReport::measured(
            "example.org",
            vec![server(DnsProbeOutcome::TimedOut, false)],
        );
        assert_eq!(
            project_dns_latency(&timeout, code).tone,
            DnsObservationTone::Danger
        );
        let ready = DnsLatencyReport::measured("example.org", Vec::new());
        let unprobed = project_dns_latency(&ready, code);
        assert_eq!(unprobed.tone, DnsObservationTone::Neutral);
        assert!(unprobed.rows.is_empty());
        let unsupported =
            project_dns_latency(&DnsLatencyReport::unsupported("not present {count}"), code);
        assert_eq!(unsupported.tone, DnsObservationTone::Neutral);
        assert!(unsupported.rows.is_empty());
        assert!(unsupported.summary.ends_with("(not present {count})"));
        assert_ne!(unsupported.empty, unprobed.empty);
    }
}
#[test]
fn probe_rows_preserve_order_and_opaque_reasons_without_replacing_user_braces() {
    let report = DnsLatencyReport::measured(
        "example.org",
        vec![
            server(DnsProbeOutcome::Measured { rtt_ms: 12 }, false),
            server(DnsProbeOutcome::TimedOut, false),
            server(
                DnsProbeOutcome::InvalidResponse {
                    reason: "bad {ms}".into(),
                },
                true,
            ),
            server(
                DnsProbeOutcome::Failed {
                    message: "socket {reason}".into(),
                },
                false,
            ),
            server(
                DnsProbeOutcome::NotProbed {
                    reason: "transport {ms}".into(),
                },
                true,
            ),
        ],
    );
    for code in ["en-US", "zh-CN"] {
        let display = project_dns_latency(&report, code);
        assert_eq!(display.rows.len(), 5);
        assert_eq!(display.tone, DnsObservationTone::Warning);
        assert_eq!(display.rows[0].outcome, "12 ms");
        assert!(display.rows[2].outcome.ends_with("bad {ms}"));
        assert!(display.rows[3].outcome.ends_with("socket {reason}"));
        assert!(display.rows[4].outcome.ends_with("transport {ms}"));
        assert_eq!(display.rows[4].tone, DnsObservationTone::Neutral);
        assert_eq!(display.listing().lines().count(), 5);
        assert_ne!(display.rows[0].tier, display.rows[2].tier);
    }
}
#[test]
fn mapping_search_preserves_observed_subset_and_distinguishes_no_matches_from_missing_feed() {
    let mut pool = FakeIpMappingPool {
        source: FakeIpMappingSource::LiveConnections,
        range: "198.18.0.1/16 {shown}".into(),
        total: 2,
        entries: vec![
            FakeIpMappingEntry {
                domain: "Music{total}.example".into(),
                address: "198.18.0.5".into(),
            },
            FakeIpMappingEntry {
                domain: "cdn.example".into(),
                address: "198.18.0.7".into(),
            },
        ],
    };
    for code in ["en-US", "zh-CN"] {
        let filtered = project_mappings(&pool, " MUSIC ", code);
        assert_eq!(filtered.rows, vec![pool.entries[0].clone()]);
        assert_eq!(filtered.listing(), "198.18.0.5 ↔ Music{total}.example");
        assert!(filtered.count.contains("1"));
        assert!(filtered.count.contains("2"));
        assert!(filtered.count.ends_with("198.18.0.1/16 {shown})"));
        let absent = project_mappings(&pool, "missing", code);
        assert!(absent.rows.is_empty());
        let mut empty = pool.clone();
        empty.entries.clear();
        empty.total = 0;
        assert_ne!(
            absent.empty,
            project_mappings(&empty, "missing", code).empty
        );
    }
    pool.source = FakeIpMappingSource::Unsupported {
        reason: "no port {shown}".into(),
    };
    let unsupported = project_mappings(&pool, "", "en-US");
    assert!(unsupported.rows.is_empty());
    assert!(unsupported.empty.ends_with("no port {shown})"));
    pool.source = FakeIpMappingSource::Unavailable {
        reason: "connection failure {shown}".into(),
    };
    let unavailable = project_mappings(&pool, "", "en-US");
    assert!(unavailable.rows.is_empty());
    assert!(unavailable.empty.ends_with("connection failure {shown})"));
    assert_ne!(unsupported.source, unavailable.source);
}
#[test]
fn health_projection_preserves_each_fact_but_incomplete_coverage_cannot_claim_overall_health() {
    let healthy = |kind| DnsSelfHealCheck {
        kind,
        state: DnsSelfHealState::Healthy,
        detail: "actual observation {fix}".into(),
        fix: None,
    };
    let mut snapshot = DnsSelfHealSnapshot::new(vec![healthy(DnsSelfHealKind::ListenPort)]);
    for code in ["en-US", "zh-CN"] {
        let display = project_dns_health(&snapshot, code);
        assert_eq!(display.tone, DnsObservationTone::Neutral);
        assert_eq!(display.rows[0].tone, DnsObservationTone::Success);
        assert_eq!(display.rows[0].detail, "actual observation {fix}");
    }
    snapshot
        .checks
        .push(healthy(DnsSelfHealKind::UpstreamResolution));
    snapshot.checks.push(healthy(DnsSelfHealKind::Topology));
    assert_eq!(
        project_dns_health(&snapshot, "en-US").tone,
        DnsObservationTone::Success
    );
    snapshot.checks[1].state = DnsSelfHealState::Unknown;
    assert_eq!(
        project_dns_health(&snapshot, "en-US").tone,
        DnsObservationTone::Neutral
    );
    snapshot.checks[0].state = DnsSelfHealState::Critical;
    snapshot.checks[0].fix = Some(DnsSelfHealFix::RepairDnsListenPort);
    let failed = project_dns_health(&snapshot, "en-US");
    assert_eq!(failed.tone, DnsObservationTone::Danger);
    assert!(
        failed.rows[0]
            .fix
            .as_ref()
            .unwrap()
            .contains("Repair the DNS listen port")
    );
    assert!(!failed.listing().contains("repair_dns_listen_port"));
    assert_eq!(failed.rows[0].detail, "actual observation {fix}");
}
