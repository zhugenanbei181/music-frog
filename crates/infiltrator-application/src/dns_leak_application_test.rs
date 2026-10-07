//! test-intent: behavior

use super::*;
use infiltrator_contract::dns_leak::{
    DnsLeakConclusion, DnsLeakEchoRecord, DnsLeakEchoReport, DnsLeakProbeName,
};
use infiltrator_contract::error::ErrorCode;
struct RecordingEcho {
    seen: Mutex<Vec<DnsLeakEchoRequest>>,
    responder: Box<dyn Fn(&DnsLeakEchoProbe) -> DnsLeakObservationOutcome + Send + Sync>,
}

impl RecordingEcho {
    fn new(
        responder: impl Fn(&DnsLeakEchoProbe) -> DnsLeakObservationOutcome + Send + Sync + 'static,
    ) -> Arc<Self> {
        Arc::new(Self {
            seen: Mutex::new(Vec::new()),
            responder: Box::new(responder),
        })
    }
}

#[async_trait::async_trait]
impl DnsLeakEchoPort for RecordingEcho {
    async fn observe(&self, request: DnsLeakEchoRequest) -> Result<DnsLeakEchoReport, PortError> {
        let observations = request
            .probes
            .iter()
            .map(|probe| DnsLeakObservation {
                resolver: probe.resolver.clone(),
                authority: probe.authority.clone(),
                question: probe.question.clone(),
                transport: DnsLeakProbeTransport::Udp,
                outcome: (self.responder)(probe),
            })
            .collect();
        self.seen.lock().expect("echo request lock").push(request);
        Ok(DnsLeakEchoReport::new(observations))
    }
}

struct PartialEcho {
    seen: Mutex<Vec<DnsLeakEchoRequest>>,
}

#[async_trait::async_trait]
impl DnsLeakEchoPort for PartialEcho {
    async fn observe(&self, request: DnsLeakEchoRequest) -> Result<DnsLeakEchoReport, PortError> {
        let first = request.probes.first().expect("one probe").clone();
        self.seen.lock().expect("echo request lock").push(request);
        Ok(DnsLeakEchoReport::new(vec![DnsLeakObservation {
            resolver: first.resolver,
            authority: first.authority,
            question: first.question,
            transport: DnsLeakProbeTransport::Udp,
            outcome: observed("203.0.113.9"),
        }]))
    }
}

struct FailingEcho;

#[async_trait::async_trait]
impl DnsLeakEchoPort for FailingEcho {
    async fn observe(&self, _request: DnsLeakEchoRequest) -> Result<DnsLeakEchoReport, PortError> {
        Err(PortError::Failed("the prober crashed".to_owned()))
    }
}

fn source(resolver: &str, authority: &str) -> DnsLeakProbeSource {
    DnsLeakProbeSource::new(resolver, authority)
}

fn observed(identity: &str) -> DnsLeakObservationOutcome {
    DnsLeakObservationOutcome::Observed {
        identity: identity.to_owned(),
    }
}

#[tokio::test]
async fn a_host_without_an_echo_prober_reports_typed_unsupported_and_probes_nothing() {
    let application = DnsLeakApplication::unconfigured();
    assert!(!application.has_echo_prober());
    assert!(!application.status().is_ready());
    assert_eq!(application.status().reason(), Some(NO_ECHO_PORT_REASON));
    assert!(!application.last_report().is_probed());
    assert_eq!(
        application.last_report().conclusion(),
        DnsLeakConclusion::Unsupported {
            reason: NO_ECHO_PORT_REASON.to_owned()
        }
    );

    let error = DnsLeakProbePort::probe(&application)
        .await
        .expect_err("no prober must refuse");
    assert_eq!(error.error_code(), ErrorCode::Unsupported);
}

#[tokio::test]
async fn a_host_without_a_configured_authority_never_claims_a_verdict() {
    let application = DnsLeakApplication::new(
        Some(RecordingEcho::new(|_| observed("203.0.113.9"))),
        Vec::new(),
    );
    assert!(application.has_echo_prober());
    assert!(!application.status().is_ready());
    assert_eq!(application.status().reason(), Some(NO_ECHO_SOURCE_REASON));
    assert!(application.last_report().sources.is_empty());
    let error = DnsLeakProbePort::probe(&application)
        .await
        .expect_err("no configured source must refuse");
    assert_eq!(error.error_code(), ErrorCode::Unsupported);
}

#[tokio::test]
async fn one_probe_generates_fresh_subdomains_and_publishes_divergent_facts() {
    let echo = RecordingEcho::new(|probe| {
        if probe.authority == "b.echo.example.org" {
            observed("198.51.100.7")
        } else {
            observed("203.0.113.9")
        }
    });
    let application = DnsLeakApplication::new(
        Some(echo.clone()),
        vec![
            source("1.1.1.1", "a.echo.example.org"),
            source("1.1.1.1", "b.echo.example.org"),
        ],
    );
    assert!(application.status().is_ready());

    let report = DnsLeakProbePort::probe(&application).await.expect("probe");
    assert!(report.is_probed());
    assert_eq!(report.observed_facts().len(), 2);
    let conclusion = report.conclusion();
    assert!(conclusion.is_divergent());
    let facts = conclusion.facts();
    assert_eq!(facts[0].identity, "203.0.113.9");
    assert_eq!(facts[1].identity, "198.51.100.7");
    assert_eq!(application.last_report(), report);

    // The generated names live under their authority and are unique.
    let seen = echo.seen.lock().expect("echo request lock");
    let questions: Vec<&str> = seen[0]
        .probes
        .iter()
        .map(|probe| probe.question.as_str())
        .collect();
    assert_eq!(questions.len(), 2);
    assert_ne!(questions[0], questions[1]);
    assert!(questions[0].ends_with(".a.echo.example.org"));
    assert!(questions[1].ends_with(".b.echo.example.org"));
    assert!(
        questions
            .iter()
            .all(|question| is_valid_probe_name(question))
    );
}

#[tokio::test]
async fn agreeing_authorities_are_consistent_and_a_silent_one_is_still_listed() {
    let echo = RecordingEcho::new(|probe| {
        if probe.resolver == "8.8.8.8" {
            DnsLeakObservationOutcome::TimedOut
        } else {
            observed("203.0.113.9")
        }
    });
    let application = DnsLeakApplication::new(
        Some(echo),
        vec![
            source("1.1.1.1", "a.echo.example.org"),
            source("1.1.1.1", "b.echo.example.org"),
            source("8.8.8.8", "a.echo.example.org"),
        ],
    );
    let report = DnsLeakProbePort::probe(&application).await.expect("probe");

    match report.conclusion() {
        DnsLeakConclusion::Consistent { identity, facts } => {
            assert_eq!(identity, "203.0.113.9");
            assert_eq!(facts.len(), 2);
            assert!(facts.iter().all(|fact| fact.identity == "203.0.113.9"));
        }
        other => panic!("agreeing sources must be consistent: {other:?}"),
    }
    assert_eq!(report.failed_count(), 1);
    assert_eq!(
        report.observations.len(),
        3,
        "the silent source stays visible"
    );
}

#[tokio::test]
async fn a_prober_that_omits_an_observation_makes_that_source_fail_honestly() {
    let partial: Arc<dyn DnsLeakEchoPort> = Arc::new(PartialEcho {
        seen: Mutex::new(Vec::new()),
    });
    let application = DnsLeakApplication::new(
        Some(partial),
        vec![
            source("1.1.1.1", "a.echo.example.org"),
            source("1.1.1.1", "b.echo.example.org"),
        ],
    );
    let report = DnsLeakProbePort::probe(&application).await.expect("probe");

    assert_eq!(report.observations.len(), 2);
    assert_eq!(report.failed_count(), 1);
    assert_eq!(report.observations[1].outcome.identity(), None);
    assert!(matches!(
        report.observations[1].outcome,
        DnsLeakObservationOutcome::Failed { .. }
    ));
    assert_eq!(
        report.conclusion(),
        DnsLeakConclusion::Unknown,
        "one observation cannot become a verdict"
    );
}

#[tokio::test]
async fn an_unusable_authority_is_not_sent_to_the_prober() {
    let echo = RecordingEcho::new(|_| observed("203.0.113.9"));
    let application = DnsLeakApplication::new(
        Some(echo.clone()),
        vec![
            source("1.1.1.1", "a.echo.example.org"),
            source("1.1.1.1", "not a zone"),
        ],
    );
    let report = DnsLeakProbePort::probe(&application).await.expect("probe");

    assert_eq!(report.observations.len(), 2);
    assert!(report.observations[1].outcome.identity().is_none());
    assert!(matches!(
        report.observations[1].transport,
        DnsLeakProbeTransport::Undrivable { .. }
    ));
    let seen = echo.seen.lock().expect("echo request lock");
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].probes.len(), 1, "the invalid source was not probed");
}

#[tokio::test]
async fn a_prober_error_never_overwrites_the_last_honest_report() {
    let application = DnsLeakApplication::new(
        Some(Arc::new(FailingEcho)),
        vec![source("1.1.1.1", "a.echo.example.org")],
    );
    let error = DnsLeakProbePort::probe(&application)
        .await
        .expect_err("the host prober failed");
    assert_eq!(error.error_code(), ErrorCode::Internal);
    assert!(!application.last_report().is_probed());
    assert!(application.last_report().observations.is_empty());
}

#[test]
fn random_probe_labels_are_unique_and_valid_dns_labels() {
    let first = random_probe_label();
    let second = random_probe_label();
    assert_ne!(first, second);
    assert!(is_valid_probe_name(&first));

    let question = random_probe_question("echo.example.org.");
    assert!(question.ends_with(".echo.example.org"));
    assert!(is_valid_probe_name(&question));
    assert_eq!(random_probe_question(""), "");

    assert!(!is_valid_probe_name(""));
    assert!(!is_valid_probe_name("a..b"));
    assert!(!is_valid_probe_name(&"x".repeat(64)));
    assert!(!is_valid_probe_name(&format!("a.{}", "x".repeat(300))));
}

#[tokio::test]
async fn an_exact_authority_source_asks_the_fixed_name_and_carries_its_record() {
    let echo = RecordingEcho::new(|_| observed("203.0.113.9"));
    let application = DnsLeakApplication::new(
        Some(echo.clone()),
        vec![DnsLeakProbeSource::exact(
            "system",
            "whoami.ds.akahelp.net",
            DnsLeakEchoRecord::TxtKeyedValue {
                key: "ip".to_owned(),
            },
        )],
    );
    assert!(application.status().is_ready());
    let report = DnsLeakProbePort::probe(&application).await.expect("probe");
    assert_eq!(report.observations.len(), 1);

    let seen = echo.seen.lock().expect("echo request lock");
    assert_eq!(
        seen[0].probes[0].question, "whoami.ds.akahelp.net",
        "a fixed-name authority is asked exactly, not under a random label"
    );
    assert_eq!(
        seen[0].probes[0].record,
        DnsLeakEchoRecord::TxtKeyedValue {
            key: "ip".to_owned()
        }
    );
}

#[test]
fn default_echo_sources_are_two_real_public_txt_authorities() {
    let sources = default_echo_sources();
    assert!(sources.len() >= 2, "cross-source corroboration needs two");
    assert!(sources.iter().all(|source| source.resolver == "system"));
    assert!(
        sources
            .iter()
            .all(|source| source.probe_name == DnsLeakProbeName::ExactAuthority)
    );
    assert!(sources.iter().any(|source| {
        matches!(&source.record, DnsLeakEchoRecord::TxtKeyedValue { key } if key == "ip")
    }));
    assert!(
        sources
            .iter()
            .any(|source| source.record == DnsLeakEchoRecord::TxtFirstIpAddress)
    );
    assert!(
        sources
            .iter()
            .any(|source| source.authority == "whoami.ds.akahelp.net")
    );
    assert!(
        sources
            .iter()
            .any(|source| source.authority == "o-o.myaddr.l.google.com")
    );

    // The default is a configuration, not a verdict: no identity is
    // pre-filled and nothing is observed until the prober runs.
    let application = DnsLeakApplication::new(
        Some(RecordingEcho::new(|_| observed("203.0.113.9"))),
        sources,
    );
    assert!(application.status().is_ready());
    assert!(application.last_report().observations.is_empty());
}
