//! DUAL-14-08: the shared DNS leak cross-source probe use-case.
//!
//! One instance is shared by the command handler and the surface reader, so
//! the last real cross-source probe reaches both surfaces. The application
//! owns the probe policy: it generates a fresh subdomain per configured
//! source (the authority is configured by the host), queries every source
//! through the injected host echo prober and compares the observed resolver
//! identities. A host without a fact source (no prober, or no configured echo
//! authority) stays a typed unsupported status; a divergent result lists the
//! observed facts and never guesses which one is a leak.

use infiltrator_contract::dns_leak::{
    DnsLeakEchoProbe, DnsLeakEchoRequest, DnsLeakObservation, DnsLeakObservationOutcome,
    DnsLeakProbeSource, DnsLeakProbeTransport, DnsLeakReport, DnsLeakStatus, MAX_PROBE_NAME_LEN,
};
use infiltrator_ports::dns_leak::{DnsLeakEchoPort, DnsLeakProbePort};
use infiltrator_ports::error::PortError;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

/// The typed reason a host without an echo prober publishes.
pub const NO_ECHO_PORT_REASON: &str = "this host injected no DNS leak echo prober";

/// The typed reason a host with a prober but no configured source publishes.
pub const NO_ECHO_SOURCE_REASON: &str =
    "no controlled DNS leak echo authority is configured on this host";

/// The typed reason one source is skipped before any exchange.
const INVALID_AUTHORITY_REASON: &str = "the configured echo authority is not a valid DNS zone name";

/// The typed reason a host prober answered fewer observations than asked.
const MISSING_OBSERVATION_REASON: &str =
    "the host echo prober returned no observation for this source";

#[derive(Clone)]
pub struct DnsLeakApplication {
    port: Option<Arc<dyn DnsLeakEchoPort>>,
    sources: Vec<DnsLeakProbeSource>,
    timeout_ms: u32,
    last: Arc<Mutex<DnsLeakReport>>,
}

impl DnsLeakApplication {
    pub fn new(port: Option<Arc<dyn DnsLeakEchoPort>>, sources: Vec<DnsLeakProbeSource>) -> Self {
        Self {
            port,
            sources,
            timeout_ms: infiltrator_contract::dns_leak::DEFAULT_ECHO_TIMEOUT_MS,
            last: Arc::new(Mutex::new(DnsLeakReport::default())),
        }
    }

    /// A host that cannot cross-check keeps the honest typed refusal.
    pub fn unconfigured() -> Self {
        Self::new(None, Vec::new())
    }

    /// Probe with a caller-supplied deadline (used by the deterministic tests).
    pub fn with_timeout_ms(mut self, timeout_ms: u32) -> Self {
        if timeout_ms > 0 {
            self.timeout_ms = timeout_ms.min(infiltrator_contract::dns_leak::MAX_ECHO_TIMEOUT_MS);
        }
        self
    }

    pub fn has_echo_prober(&self) -> bool {
        self.port.is_some()
    }

    pub fn sources(&self) -> &[DnsLeakProbeSource] {
        &self.sources
    }

    /// Whether this host can produce real cross-source leak facts.
    pub fn status(&self) -> DnsLeakStatus {
        if self.port.is_none() {
            return DnsLeakStatus::unsupported(NO_ECHO_PORT_REASON);
        }
        if self.sources.is_empty() {
            return DnsLeakStatus::unsupported(NO_ECHO_SOURCE_REASON);
        }
        DnsLeakStatus::Ready
    }

    /// The last probe, with the status and configured sources re-derived from
    /// the application so a host can never publish a stale readiness.
    pub fn last_report(&self) -> DnsLeakReport {
        let mut report = self.last.lock().expect("dns leak report lock").clone();
        report.status = self.status();
        report.sources = self.sources.clone();
        report
    }

    /// Ask the host echo prober for every configured source, then publish the
    /// one shared report. The random subdomains are generated here, so two
    /// surfaces can never probe different questions.
    async fn run_probe(&self, port: &Arc<dyn DnsLeakEchoPort>) -> Result<DnsLeakReport, PortError> {
        let mut pending: Vec<(usize, DnsLeakEchoProbe)> = Vec::new();
        let mut observations: Vec<Option<DnsLeakObservation>> =
            (0..self.sources.len()).map(|_| None).collect();
        for (index, source) in self.sources.iter().enumerate() {
            let question = random_probe_question(&source.authority);
            if !is_valid_probe_name(&question) {
                observations[index] = Some(DnsLeakObservation {
                    resolver: source.resolver.clone(),
                    authority: source.authority.clone(),
                    question,
                    transport: DnsLeakProbeTransport::Undrivable {
                        reason: INVALID_AUTHORITY_REASON.to_owned(),
                    },
                    outcome: DnsLeakObservationOutcome::NotProbed {
                        reason: INVALID_AUTHORITY_REASON.to_owned(),
                    },
                });
                continue;
            }
            pending.push((
                index,
                DnsLeakEchoProbe {
                    resolver: source.resolver.clone(),
                    authority: source.authority.clone(),
                    question,
                },
            ));
        }

        if !pending.is_empty() {
            let proposal: Vec<DnsLeakEchoProbe> =
                pending.iter().map(|(_, probe)| probe.clone()).collect();
            let request = DnsLeakEchoRequest::new(proposal).with_timeout_ms(self.timeout_ms);
            let echo = port.observe(request).await?;
            let mut returned = echo.observations;
            for (index, probe) in pending {
                let position = returned.iter().position(|observation| {
                    observation.question == probe.question
                        && observation.resolver == probe.resolver
                        && observation.authority == probe.authority
                });
                observations[index] = Some(match position {
                    Some(position) => returned.remove(position),
                    None => DnsLeakObservation {
                        resolver: probe.resolver.clone(),
                        authority: probe.authority.clone(),
                        question: probe.question.clone(),
                        transport: DnsLeakProbeTransport::Undrivable {
                            reason: MISSING_OBSERVATION_REASON.to_owned(),
                        },
                        outcome: DnsLeakObservationOutcome::Failed {
                            message: MISSING_OBSERVATION_REASON.to_owned(),
                        },
                    },
                });
            }
        }

        let report = DnsLeakReport::observed(
            self.sources.clone(),
            observations.into_iter().flatten().collect(),
        );
        *self.last.lock().expect("dns leak report lock") = report.clone();
        Ok(report)
    }
}

#[async_trait::async_trait]
impl DnsLeakProbePort for DnsLeakApplication {
    async fn probe(&self) -> Result<DnsLeakReport, PortError> {
        let Some(port) = self.port.as_ref() else {
            let report = DnsLeakReport::unsupported(NO_ECHO_PORT_REASON);
            *self.last.lock().expect("dns leak report lock") = report;
            return Err(PortError::unsupported(
                infiltrator_contract::capability::Capability::Dns,
                NO_ECHO_PORT_REASON,
            ));
        };
        if self.sources.is_empty() {
            let report = DnsLeakReport::unsupported(NO_ECHO_SOURCE_REASON);
            *self.last.lock().expect("dns leak report lock") = report;
            return Err(PortError::unsupported(
                infiltrator_contract::capability::Capability::Dns,
                NO_ECHO_SOURCE_REASON,
            ));
        }
        self.run_probe(port).await
    }
}

/// The question of a valid probe candidate: a fresh subdomain under the
/// configured authority.
pub fn random_probe_question(authority: &str) -> String {
    let authority = authority.trim().trim_end_matches('.');
    if authority.is_empty() {
        return String::new();
    }
    format!("{}.{authority}", random_probe_label())
}

/// A cache-busting nonce for the probe label. It only has to be unique enough
/// that a resolver cache cannot answer the previous probe; it is not a
/// security token, so the workspace CSPRNG dependency is not pulled in.
pub fn random_probe_label() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos() as u64)
        .unwrap_or(0);
    let counter = NEXT.fetch_add(1, Ordering::Relaxed);
    let mixed = splitmix64(nanos ^ counter.wrapping_mul(0x9E37_79B9_7F4A_7C15));
    let mixed = splitmix64(mixed ^ u64::from(std::process::id()));
    format!("l{mixed:016x}")
}

fn splitmix64(value: u64) -> u64 {
    let mut state = value.wrapping_add(0x9E37_79B9_7F4A_7C15);
    state = (state ^ (state >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    state = (state ^ (state >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    state ^ (state >> 31)
}

/// Whether a generated name can be asked of a real authority.
pub fn is_valid_probe_name(name: &str) -> bool {
    if name.is_empty() || name.len() > MAX_PROBE_NAME_LEN {
        return false;
    }
    name.split('.').all(|label| {
        !label.is_empty()
            && label.len() <= 63
            && label.chars().all(|character| {
                character.is_ascii_alphanumeric() || character == '-' || character == '_'
            })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::dns_leak::{DnsLeakConclusion, DnsLeakEchoReport};
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
        async fn observe(
            &self,
            request: DnsLeakEchoRequest,
        ) -> Result<DnsLeakEchoReport, PortError> {
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
        async fn observe(
            &self,
            request: DnsLeakEchoRequest,
        ) -> Result<DnsLeakEchoReport, PortError> {
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
        async fn observe(
            &self,
            _request: DnsLeakEchoRequest,
        ) -> Result<DnsLeakEchoReport, PortError> {
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
}
