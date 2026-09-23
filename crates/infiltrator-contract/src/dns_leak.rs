//! DUAL-14-08: the shared DNS leak cross-source probe contract.
//!
//! A leak *conclusion* is never authored by a surface. The host resolves a
//! freshly generated subdomain under a controlled echo authority through a
//! configured resolver; the authority answers with the resolver identity it
//! actually observed. The application compares the observed identities of
//! every configured source and publishes one of five typed conclusions:
//!
//! * [`DnsLeakConclusion::Unknown`] — not enough cross-source facts yet;
//! * [`DnsLeakConclusion::Consistent`] — every accepted source observed the
//!   same resolver identity;
//! * [`DnsLeakConclusion::Divergent`] — sources observed different resolver
//!   identities; the report lists every observed fact and never guesses which
//!   one is "the" leak;
//! * [`DnsLeakConclusion::Unsupported`] — this host has no echo fact source;
//! * [`DnsLeakConclusion::Failed`] — sources were attempted and none produced
//!   an observation.
//!
//! Everything both surfaces render comes from these types.

use serde::{Deserialize, Serialize};

/// Per-source deadline shared by the UDP/DoH/system transports.
pub const DEFAULT_ECHO_TIMEOUT_MS: u32 = 2_000;

/// Highest deadline an echo probe accepts; a larger request is clamped so a
/// stalled source can never widen the shared probe window.
pub const MAX_ECHO_TIMEOUT_MS: u32 = 60_000;

/// Longest generated subdomain label. Kept well under the 63 byte DNS label
/// limit so the authority can always prepend its own prefix if it wants to.
pub const MAX_PROBE_LABEL_LEN: usize = 40;

/// Longest fully qualified probe name (RFC 1035 §2.3.4).
pub const MAX_PROBE_NAME_LEN: usize = 253;

/// Honest DNS leak cross-source probe availability.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DnsLeakStatus {
    /// A real echo prober and at least one configured probe source exist on
    /// this host.
    Ready,
    /// Nothing has been reported yet: the host neither confirmed a prober nor
    /// refused one, so the report must stay a non-verdict.
    #[default]
    Unknown,
    /// No host fact source reports a resolver identity; no surface may invent
    /// a leak verdict.
    Unsupported { reason: String },
}

impl DnsLeakStatus {
    pub fn unsupported(reason: &str) -> Self {
        Self::Unsupported {
            reason: reason.to_owned(),
        }
    }

    pub const fn is_ready(&self) -> bool {
        matches!(self, Self::Ready)
    }

    /// The typed refusal reason, when the host cannot cross-check.
    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::Ready | Self::Unknown => None,
            Self::Unsupported { reason } => Some(reason),
        }
    }
}

/// One configured probe source: resolve a fresh subdomain under `authority`
/// through `resolver` and report the identity the authority observed.
///
/// The intended configuration is one resolution path observed by two or more
/// independent authorities, so agreement is real corroboration. The report
/// always lists the resolver and authority of every fact, so a differently
/// configured source is visible instead of being averaged away.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DnsLeakProbeSource {
    /// The resolver whose egress identity is observed (`ip:port`, a DoH URL
    /// or the platform resolver `system`).
    pub resolver: String,
    /// The controlled echo authority zone the random subdomain lives under.
    pub authority: String,
}

impl DnsLeakProbeSource {
    pub fn new(resolver: &str, authority: &str) -> Self {
        Self {
            resolver: resolver.trim().to_owned(),
            authority: authority.trim().trim_end_matches('.').to_owned(),
        }
    }
}

/// The transport one echo observation used.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DnsLeakProbeTransport {
    /// Plain DNS over UDP.
    Udp,
    /// DNS over HTTPS (wire format) through the host HTTP client.
    Doh,
    /// The platform resolver (the path a DNS leak would actually flow
    /// through), observed through a host name lookup.
    System,
    /// This host cannot drive the resolver address (DoT / DoQ / DNSCrypt /
    /// DHCP, or an HTTP/3 endpoint without an h3 client).
    Undrivable { reason: String },
}

/// What one configured source produced.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DnsLeakObservationOutcome {
    /// The authority answered with a resolver identity it observed.
    Observed { identity: String },
    /// No answer arrived before the deadline.
    TimedOut,
    /// An answer arrived but carried no usable observation (wrong id, wrong
    /// question, an error response code, or no answer record).
    InvalidResponse { reason: String },
    /// The transport ran and failed (socket, HTTP, or lookup error).
    Failed { message: String },
    /// The source was not probed at all.
    NotProbed { reason: String },
}

impl DnsLeakObservationOutcome {
    /// The observed resolver identity, or `None` when nothing was observed.
    pub fn identity(&self) -> Option<&str> {
        match self {
            Self::Observed { identity } => Some(identity),
            _ => None,
        }
    }

    pub const fn is_observed(&self) -> bool {
        matches!(self, Self::Observed { .. })
    }
}

/// One configured source's last echo result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DnsLeakObservation {
    pub resolver: String,
    pub authority: String,
    /// The random subdomain this observation was asked about, so a result is
    /// never detached from the exact question that produced it.
    pub question: String,
    pub transport: DnsLeakProbeTransport,
    pub outcome: DnsLeakObservationOutcome,
}

impl DnsLeakObservation {
    pub fn observed(
        resolver: &str,
        authority: &str,
        question: &str,
        transport: DnsLeakProbeTransport,
        identity: &str,
    ) -> Self {
        Self {
            resolver: resolver.to_owned(),
            authority: authority.to_owned(),
            question: question.to_owned(),
            transport,
            outcome: DnsLeakObservationOutcome::Observed {
                identity: identity.to_owned(),
            },
        }
    }
}

/// One resolver identity a real authority observed. This is the fact a
/// divergent report lists; it is never a guess.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DnsLeakObservedFact {
    pub resolver: String,
    pub authority: String,
    pub identity: String,
}

/// The typed cross-source conclusion both surfaces render.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DnsLeakConclusion {
    /// Fewer than two sources produced an observation: not enough independent
    /// facts to conclude anything.
    Unknown,
    /// This host has no echo fact source (no prober / no configured source).
    Unsupported { reason: String },
    /// Every accepted source observed the same resolver identity.
    Consistent {
        identity: String,
        facts: Vec<DnsLeakObservedFact>,
    },
    /// Sources observed different resolver identities. All observed facts are
    /// listed; the conclusion itself stays a fact report, not a verdict about
    /// which identity is a leak.
    Divergent { facts: Vec<DnsLeakObservedFact> },
    /// Sources were attempted and none produced an observation.
    Failed { reason: String },
}

impl DnsLeakConclusion {
    pub const fn is_divergent(&self) -> bool {
        matches!(self, Self::Divergent { .. })
    }

    pub const fn is_consistent(&self) -> bool {
        matches!(self, Self::Consistent { .. })
    }

    /// Every observed fact the conclusion carries, for both agreed and
    /// disagreeing sources.
    pub fn facts(&self) -> &[DnsLeakObservedFact] {
        match self {
            Self::Consistent { facts, .. } | Self::Divergent { facts } => facts,
            _ => &[],
        }
    }

    /// The agreed identity, when the sources agreed.
    pub fn agreed_identity(&self) -> Option<&str> {
        match self {
            Self::Consistent { identity, .. } => Some(identity),
            _ => None,
        }
    }
}

/// The reason published when no two sources ever agreed on a fact.
pub const NO_OBSERVATION_REASON: &str =
    "no configured DNS leak probe source produced an observation";

/// The last DNS leak cross-source probe of this host.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DnsLeakReport {
    pub status: DnsLeakStatus,
    /// The sources the host was configured with, whether or not this session
    /// already probed them.
    pub sources: Vec<DnsLeakProbeSource>,
    pub observations: Vec<DnsLeakObservation>,
}

impl DnsLeakReport {
    /// A report for a host that cannot cross-check, carrying the typed reason.
    pub fn unsupported(reason: &str) -> Self {
        Self {
            status: DnsLeakStatus::unsupported(reason),
            ..Self::default()
        }
    }

    /// A report for a configured host whose sources were actually probed.
    pub fn observed(
        sources: Vec<DnsLeakProbeSource>,
        observations: Vec<DnsLeakObservation>,
    ) -> Self {
        Self {
            status: DnsLeakStatus::Ready,
            sources,
            observations,
        }
    }

    /// Whether this report carries results from a host that can cross-check.
    pub fn is_probed(&self) -> bool {
        self.status.is_ready() && !self.observations.is_empty()
    }

    /// Every identity a real authority reported, in observation order and
    /// de-duplicated by (resolver, authority, identity).
    pub fn observed_facts(&self) -> Vec<DnsLeakObservedFact> {
        let mut facts: Vec<DnsLeakObservedFact> = Vec::new();
        for observation in &self.observations {
            let Some(identity) = observation.outcome.identity() else {
                continue;
            };
            let fact = DnsLeakObservedFact {
                resolver: observation.resolver.clone(),
                authority: observation.authority.clone(),
                identity: identity.to_owned(),
            };
            if !facts.contains(&fact) {
                facts.push(fact);
            }
        }
        facts
    }

    /// The typed cross-source conclusion, derived from the observed facts.
    pub fn conclusion(&self) -> DnsLeakConclusion {
        // Nothing reported yet is not a verdict; the host has neither confirmed
        // a prober nor refused one.
        if matches!(self.status, DnsLeakStatus::Unknown) {
            return DnsLeakConclusion::Unknown;
        }
        let DnsLeakStatus::Unsupported { reason } = &self.status else {
            let facts = self.observed_facts();
            if facts.is_empty() {
                return if self.observations.is_empty() {
                    DnsLeakConclusion::Unknown
                } else {
                    DnsLeakConclusion::Failed {
                        reason: NO_OBSERVATION_REASON.to_owned(),
                    }
                };
            }
            // One observation is a single fact, not cross-source agreement.
            if facts.len() < 2 {
                return DnsLeakConclusion::Unknown;
            }
            let identity = &facts[0].identity;
            if facts.iter().all(|fact| &fact.identity == identity) {
                return DnsLeakConclusion::Consistent {
                    identity: identity.clone(),
                    facts,
                };
            }
            return DnsLeakConclusion::Divergent { facts };
        };
        DnsLeakConclusion::Unsupported {
            reason: reason.clone(),
        }
    }

    /// How many sources produced no observation.
    pub fn failed_count(&self) -> usize {
        self.observations
            .iter()
            .filter(|observation| !observation.outcome.is_observed())
            .count()
    }
}

/// One source to echo-probe, carrying the already generated subdomain.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DnsLeakEchoProbe {
    pub resolver: String,
    pub authority: String,
    /// The freshly generated subdomain under `authority`.
    pub question: String,
}

/// The request handed to a host echo prober.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DnsLeakEchoRequest {
    pub probes: Vec<DnsLeakEchoProbe>,
    pub timeout_ms: u32,
}

impl DnsLeakEchoRequest {
    pub fn new(probes: Vec<DnsLeakEchoProbe>) -> Self {
        Self {
            probes,
            timeout_ms: DEFAULT_ECHO_TIMEOUT_MS,
        }
    }

    /// Clamp the deadline into a usable range (0 would make every probe time
    /// out immediately, so it is never accepted).
    pub fn with_timeout_ms(mut self, timeout_ms: u32) -> Self {
        if timeout_ms > 0 {
            self.timeout_ms = timeout_ms.min(MAX_ECHO_TIMEOUT_MS);
        }
        self
    }
}

/// What the host echo prober observed, one entry per attempted source.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DnsLeakEchoReport {
    pub observations: Vec<DnsLeakObservation>,
}

impl DnsLeakEchoReport {
    pub fn new(observations: Vec<DnsLeakObservation>) -> Self {
        Self { observations }
    }

    /// The identity observed for one exact question, if it was observed.
    pub fn identity_of(&self, question: &str) -> Option<&str> {
        self.observations
            .iter()
            .find(|observation| observation.question == question)
            .and_then(|observation| observation.outcome.identity())
    }
}

#[cfg(test)]
mod tests {
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
        let probe = DnsLeakEchoProbe {
            resolver: "1.1.1.1".to_owned(),
            authority: "echo.example.org".to_owned(),
            question: "l7f3.echo.example.org".to_owned(),
        };
        let request = DnsLeakEchoRequest::new(vec![probe.clone()]);
        assert_eq!(request.timeout_ms, DEFAULT_ECHO_TIMEOUT_MS);
        assert_eq!(request.probes[0], probe);
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
        let encoded = serde_json::to_string(&source).expect("serialize");
        assert!(encoded.contains("echo.example.org"));
        let decoded: DnsLeakProbeSource = serde_json::from_str(&encoded).expect("deserialize");
        assert_eq!(decoded, source);
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
}
