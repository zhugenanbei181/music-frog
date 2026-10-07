//! test-intent: behavior

use super::*;

fn source(resolver: &str, authority: &str) -> DnsLeakProbeSource {
    DnsLeakProbeSource::new(resolver, authority)
}

fn observation(
    resolver: &str,
    authority: &str,
    question: &str,
    identity: Option<&str>,
) -> DnsLeakObservation {
    let outcome = match identity {
        Some(identity) => DnsLeakObservationOutcome::Observed {
            identity: identity.to_owned(),
        },
        None => DnsLeakObservationOutcome::TimedOut,
    };
    DnsLeakObservation {
        resolver: resolver.to_owned(),
        authority: authority.to_owned(),
        question: question.to_owned(),
        transport: DnsLeakProbeTransport::Udp,
        outcome,
    }
}

#[test]
fn a_default_report_is_unknown_and_never_a_verdict() {
    let report = DnsLeakReport::default();
    assert!(!report.is_probed());
    assert!(!report.status.is_ready());
    assert_eq!(report.failed_count(), 0);
    assert!(report.observed_facts().is_empty());
    assert_eq!(report.conclusion(), DnsLeakConclusion::Unknown);
    assert!(!report.conclusion().is_divergent());
    assert!(!report.conclusion().is_consistent());

    let unsupported = DnsLeakReport::unsupported("no echo authority is configured");
    assert!(!unsupported.status.is_ready());
    assert_eq!(
        unsupported.status.reason(),
        Some("no echo authority is configured")
    );
    assert_eq!(
        unsupported.conclusion(),
        DnsLeakConclusion::Unsupported {
            reason: "no echo authority is configured".to_owned()
        }
    );
}

#[test]
fn a_single_observation_is_unknown_not_consistent() {
    let report = DnsLeakReport::observed(
        vec![source("1.1.1.1", "echo.example.org")],
        vec![observation(
            "1.1.1.1",
            "echo.example.org",
            "l9f3.echo.example.org",
            Some("203.0.113.9"),
        )],
    );
    assert!(report.is_probed());
    assert_eq!(report.observed_facts().len(), 1);
    assert_eq!(
        report.conclusion(),
        DnsLeakConclusion::Unknown,
        "one fact is not cross-source agreement"
    );
}

#[test]
fn agreeing_authorities_are_consistent_with_the_real_identity() {
    let report = DnsLeakReport::observed(
        vec![
            source("1.1.1.1", "a.echo.example.org"),
            source("1.1.1.1", "b.echo.example.org"),
        ],
        vec![
            observation(
                "1.1.1.1",
                "a.echo.example.org",
                "l11.a.echo.example.org",
                Some("203.0.113.9"),
            ),
            observation(
                "1.1.1.1",
                "b.echo.example.org",
                "l22.b.echo.example.org",
                Some("203.0.113.9"),
            ),
        ],
    );
    let conclusion = report.conclusion();
    assert!(conclusion.is_consistent());
    assert_eq!(conclusion.agreed_identity(), Some("203.0.113.9"));
    assert_eq!(conclusion.facts().len(), 2);
    assert_eq!(report.failed_count(), 0);
}

#[test]
fn disagreeing_authorities_are_divergent_and_list_every_fact() {
    let report = DnsLeakReport::observed(
        vec![
            source("1.1.1.1", "a.echo.example.org"),
            source("1.1.1.1", "b.echo.example.org"),
            source("system", "a.echo.example.org"),
        ],
        vec![
            observation(
                "1.1.1.1",
                "a.echo.example.org",
                "l11.a.echo.example.org",
                Some("203.0.113.9"),
            ),
            observation(
                "1.1.1.1",
                "b.echo.example.org",
                "l22.b.echo.example.org",
                Some("198.51.100.7"),
            ),
            observation(
                "system",
                "a.echo.example.org",
                "l33.a.echo.example.org",
                None,
            ),
        ],
    );
    let conclusion = report.conclusion();
    assert!(conclusion.is_divergent());
    assert_eq!(conclusion.agreed_identity(), None);
    let facts = conclusion.facts();
    assert_eq!(facts.len(), 2, "both identities are listed, not guessed");
    assert_eq!(facts[0].authority, "a.echo.example.org");
    assert_eq!(facts[0].identity, "203.0.113.9");
    assert_eq!(facts[1].resolver, "1.1.1.1");
    assert_eq!(facts[1].identity, "198.51.100.7");
    assert!(report.failed_count() == 1);
}

#[test]
fn an_all_failed_probe_is_failed_not_unknown() {
    let report = DnsLeakReport::observed(
        vec![source("1.1.1.1", "echo.example.org")],
        vec![observation(
            "1.1.1.1",
            "echo.example.org",
            "l11.echo.example.org",
            None,
        )],
    );
    assert_eq!(
        report.conclusion(),
        DnsLeakConclusion::Failed {
            reason: NO_OBSERVATION_REASON.to_owned()
        }
    );
    // A host without sources cannot be "failed": nothing was attempted.
    let untouched = DnsLeakReport::observed(Vec::new(), Vec::new());
    assert_eq!(untouched.conclusion(), DnsLeakConclusion::Unknown);
}

#[test]
fn an_unsupported_status_overrides_partial_observations() {
    let mut report = DnsLeakReport::observed(
        vec![source("1.1.1.1", "echo.example.org")],
        vec![observation(
            "1.1.1.1",
            "echo.example.org",
            "l11.echo.example.org",
            Some("203.0.113.9"),
        )],
    );
    report.status = DnsLeakStatus::unsupported("the prober was withdrawn");
    assert_eq!(
        report.conclusion(),
        DnsLeakConclusion::Unsupported {
            reason: "the prober was withdrawn".to_owned()
        }
    );
    assert!(report.observed_facts().len() == 1);
}

#[test]
fn echo_requests_keep_a_usable_deadline_and_the_exact_question() {
    let probe = DnsLeakEchoProbe::new(
        "1.1.1.1",
        "echo.example.org",
        "l7f3.echo.example.org",
        DnsLeakEchoRecord::FirstAddress,
    );
    let request = DnsLeakEchoRequest::new(vec![probe.clone()]);
    assert_eq!(request.timeout_ms, DEFAULT_ECHO_TIMEOUT_MS);
    assert_eq!(request.probes[0], probe);
    assert!(!request.probes[0].record.is_txt());
    assert_eq!(
        DnsLeakEchoRequest::new(Vec::new())
            .with_timeout_ms(0)
            .timeout_ms,
        DEFAULT_ECHO_TIMEOUT_MS
    );
    assert_eq!(
        DnsLeakEchoRequest::new(Vec::new())
            .with_timeout_ms(900_000)
            .timeout_ms,
        MAX_ECHO_TIMEOUT_MS
    );

    let report = DnsLeakEchoReport::new(vec![observation(
        "1.1.1.1",
        "echo.example.org",
        "l7f3.echo.example.org",
        Some("203.0.113.9"),
    )]);
    assert_eq!(
        report.identity_of("l7f3.echo.example.org"),
        Some("203.0.113.9")
    );
    assert_eq!(report.identity_of("other.echo.example.org"), None);
}

#[test]
fn probe_sources_trim_the_authority_dot_and_the_resolver() {
    let source = source("  1.1.1.1  ", " echo.example.org. ");
    assert_eq!(source.resolver, "1.1.1.1");
    assert_eq!(source.authority, "echo.example.org");
    assert_eq!(source.record, DnsLeakEchoRecord::FirstAddress);
    assert_eq!(source.probe_name, DnsLeakProbeName::FreshSubdomain);
    let encoded = serde_json::to_string(&source).expect("serialize");
    assert!(encoded.contains("echo.example.org"));
    let decoded: DnsLeakProbeSource = serde_json::from_str(&encoded).expect("deserialize");
    assert_eq!(decoded, source);
}

#[test]
fn echo_record_rules_are_declared_and_round_trip() {
    let first = DnsLeakEchoRecord::TxtFirstValue;
    let keyed = DnsLeakEchoRecord::TxtKeyedValue {
        key: "ip".to_owned(),
    };
    assert!(!DnsLeakEchoRecord::FirstAddress.is_txt());
    assert!(first.is_txt());
    assert!(keyed.is_txt());

    for record in [
        DnsLeakEchoRecord::FirstAddress,
        first.clone(),
        DnsLeakEchoRecord::TxtFirstIpAddress,
        keyed.clone(),
    ] {
        let encoded = serde_json::to_string(&record).expect("serialize record");
        let decoded: DnsLeakEchoRecord =
            serde_json::from_str(&encoded).expect("deserialize record");
        assert_eq!(decoded, record);
    }

    let exact = DnsLeakProbeSource::exact("system", "whoami.ds.akahelp.net", keyed.clone());
    assert_eq!(exact.probe_name, DnsLeakProbeName::ExactAuthority);
    assert_eq!(exact.record, keyed);
    assert_eq!(exact.resolver, "system");
    assert_eq!(exact.authority, "whoami.ds.akahelp.net");

    let fresh = DnsLeakProbeSource::with_record("system", "a.example.org", first.clone());
    assert_eq!(fresh.probe_name, DnsLeakProbeName::FreshSubdomain);
    assert_eq!(fresh.record, first);
}

#[test]
fn the_observation_outcome_partitions_observed_from_failed() {
    assert_eq!(
        DnsLeakObservationOutcome::Observed {
            identity: "203.0.113.9".to_owned()
        }
        .identity(),
        Some("203.0.113.9")
    );
    assert!(
        DnsLeakObservationOutcome::Observed {
            identity: "203.0.113.9".to_owned()
        }
        .is_observed()
    );
    assert_eq!(DnsLeakObservationOutcome::TimedOut.identity(), None);
    assert!(
        !DnsLeakObservationOutcome::InvalidResponse {
            reason: "no A record".to_owned()
        }
        .is_observed()
    );
}
