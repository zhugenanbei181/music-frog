//! test-intent: behavior
use std::borrow::Cow;

use crate::dns_health_projection::project_dns_health;
use crate::dns_latency_projection::project_dns_latency;
use crate::dns_settings_projection::{field_label, form_issue, form_status};
use crate::latency_projection::{project_measured_latency, project_proxy_latency};
use crate::stun_projection::project_stun;
use infiltrator_contract::dns_form::{DnsFormField, DnsFormIssue};
use infiltrator_contract::dns_latency::{
    DnsLatencyReport, DnsProbeOutcome, DnsProbeTransport, DnsServerLatency,
};
use infiltrator_contract::dns_self_heal::{
    DnsSelfHealCheck, DnsSelfHealFix, DnsSelfHealKind, DnsSelfHealSnapshot, DnsSelfHealState,
};
use infiltrator_contract::latency_display::LatencyBand;
use infiltrator_contract::stun_probe::{
    StunMappedAddress, StunProbeObservation, StunProbeReport, StunProbeStatus,
};
use infiltrator_shared::dns_observation::DnsObservationTone;
use infiltrator_shared::locales::{Lang, Localizer};
use lurker_ui_presentation::copy::TextCatalog;

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
        let display = project_dns_latency(&all, &Lang(code));
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
            project_dns_latency(&timeout, &Lang(code)).tone,
            DnsObservationTone::Danger
        );
        let ready = DnsLatencyReport::measured("example.org", Vec::new());
        let unprobed = project_dns_latency(&ready, &Lang(code));
        assert_eq!(unprobed.tone, DnsObservationTone::Neutral);
        assert!(unprobed.rows.is_empty());
        let unsupported = project_dns_latency(
            &DnsLatencyReport::unsupported("not present {count}"),
            &Lang(code),
        );
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
        let display = project_dns_latency(&report, &Lang(code));
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
fn health_projection_preserves_each_fact_but_incomplete_coverage_cannot_claim_overall_health() {
    let healthy = |kind| DnsSelfHealCheck {
        kind,
        state: DnsSelfHealState::Healthy,
        detail: "actual observation {fix}".into(),
        fix: None,
    };
    let mut snapshot = DnsSelfHealSnapshot::new(vec![healthy(DnsSelfHealKind::ListenPort)]);
    for code in ["en-US", "zh-CN"] {
        let display = project_dns_health(&snapshot, &Lang(code));
        assert_eq!(display.tone, DnsObservationTone::Neutral);
        assert_eq!(display.rows[0].tone, DnsObservationTone::Success);
        assert_eq!(display.rows[0].detail, "actual observation {fix}");
    }
    snapshot
        .checks
        .push(healthy(DnsSelfHealKind::UpstreamResolution));
    snapshot.checks.push(healthy(DnsSelfHealKind::Topology));
    assert_eq!(
        project_dns_health(&snapshot, &Lang("en-US")).tone,
        DnsObservationTone::Success
    );
    snapshot.checks[1].state = DnsSelfHealState::Unknown;
    assert_eq!(
        project_dns_health(&snapshot, &Lang("en-US")).tone,
        DnsObservationTone::Neutral
    );
    snapshot.checks[0].state = DnsSelfHealState::Critical;
    snapshot.checks[0].fix = Some(DnsSelfHealFix::RepairDnsListenPort);
    let failed = project_dns_health(&snapshot, &Lang("en-US"));
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

#[test]
fn settings_projection_keeps_validation_copy_and_placeholders_opaque_in_both_languages() {
    let entry = "ftp://{field}/{reason}/中文🙂";
    let issue = DnsFormIssue::UnsupportedScheme {
        field: DnsFormField::Nameserver,
        entry: entry.into(),
    };
    assert_eq!(
        form_issue(&issue, &Lang("en-US")),
        format!("Unsupported upstream scheme in nameserver: {entry}")
    );
    assert_eq!(
        form_issue(&issue, &Lang("zh-CN")),
        format!("字段 nameserver 的上游协议不受支持: {entry}")
    );
    assert_eq!(
        form_status(&[issue], false, true, false, &Lang("en-US")),
        format!("Local validation failed: Unsupported upstream scheme in nameserver: {entry}")
    );
    assert_eq!(
        field_label(DnsFormField::BootstrapNameserver, &Lang("en-US")),
        "default_nameserver (bootstrap, pure IP)"
    );
    assert_eq!(
        form_status(&[], false, true, true, &Lang("en-US")),
        "Command service is not composed; DNS patch was not submitted"
    );
    assert_eq!(
        form_status(&[], false, true, false, &Lang("en-US")),
        "DNS workbench patch queued; awaiting actual result"
    );
}

#[test]
fn latency_projection_keeps_measured_zero_history_zero_and_absence_distinct() {
    for language in ["en-US", "zh-CN"] {
        let catalog = Lang(language);
        let translate = |key: &str| catalog.tr(key).into_owned();
        let measured_zero = project_measured_latency(Some(0));
        assert_eq!(measured_zero.band, LatencyBand::Fast);
        assert_eq!(measured_zero.render(&catalog), "0 ms");
        let history_zero = project_proxy_latency(Some(0));
        assert_eq!(history_zero.band, LatencyBand::UnconfirmedZero);
        assert_eq!(
            history_zero.render(&catalog),
            translate("proxy_inspection_unconfirmed_zero")
        );
        let missing = project_proxy_latency(None);
        assert_eq!(missing.band, LatencyBand::NotObserved);
        assert_eq!(
            missing.render(&catalog),
            translate("proxy_inspection_unmeasured")
        );
        assert_ne!(history_zero.render(&catalog), missing.render(&catalog));
    }
}

#[test]
fn stun_projection_keeps_observation_and_egress_comparison_as_separate_facts() {
    let observed = StunProbeReport::from_observation(
        StunProbeObservation::observed(
            "stun{mapping}.example:3478",
            StunMappedAddress::new("2001:db8::7", 51000),
        ),
        Some(StunMappedAddress::new("2001:db8::7", 0)),
    );
    for code in ["en-US", "zh-CN"] {
        let display = project_stun(&observed, &Lang(code));
        assert_eq!(display.tone, DnsObservationTone::Success);
        assert!(display.status.contains("[2001:db8::7]:51000"));
        assert!(display.server.ends_with("stun{mapping}.example:3478"));
        assert_eq!(display.listing().lines().count(), 3);

        let unknown = StunProbeReport::from_observation(
            StunProbeObservation::observed(
                "stun{mapping}.example:3478",
                StunMappedAddress::new("2001:db8::7", 51000),
            ),
            None,
        );
        assert_ne!(
            project_stun(&unknown, &Lang(code)).comparison,
            display.comparison
        );
        let missing = StunProbeReport::from_observation(
            StunProbeObservation {
                status: StunProbeStatus::Observed,
                ..StunProbeObservation::default()
            },
            None,
        );
        assert_eq!(
            project_stun(&missing, &Lang(code)).tone,
            DnsObservationTone::Neutral
        );
        let failed = StunProbeReport::from_observation(
            StunProbeObservation::failed("stun.example:3478", "socket {mapping}"),
            None,
        );
        let failure = project_stun(&failed, &Lang(code));
        assert_eq!(failure.tone, DnsObservationTone::Danger);
        assert!(failure.status.ends_with("socket {mapping}"));
    }
}

/// Catalog stub that returns copy no real locale table contains, so the test
/// proves the projection reads keys through the port instead of a locale code.
struct StubCatalog;

impl TextCatalog for StubCatalog {
    fn text(&self, key: &str) -> Cow<'static, str> {
        match key {
            "dns_self_heal_healthy" => "stub-healthy".into(),
            "dns_self_heal_unknown" => "stub-unknown".into(),
            "dns_self_heal_empty" => "stub-empty {count}".into(),
            other => other.to_owned().into(),
        }
    }
}

#[test]
fn dns_health_resolves_copy_through_the_text_catalog_port() {
    let snapshot = DnsSelfHealSnapshot::new(vec![DnsSelfHealCheck {
        kind: DnsSelfHealKind::ListenPort,
        state: DnsSelfHealState::Healthy,
        detail: "observed".into(),
        fix: None,
    }]);
    let display = project_dns_health(&snapshot, &StubCatalog);
    assert_eq!(display.overall, "stub-unknown");
    assert_eq!(display.rows[0].kind, "dns_self_heal_listen_port");
    assert_eq!(display.rows[0].state, "stub-healthy");
    assert_eq!(display.empty, "stub-empty {count}");
}
