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

use crate::error::Failure;
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

/// How an echo authority's answer is read into a resolver identity.
///
/// The host *declares* the extraction rule; the adapter never guesses it from
/// whatever the answer happens to contain. An answer that does not carry the
/// declared record — or carries more than one distinct candidate — is a typed
/// [`DnsLeakObservationOutcome::InvalidResponse`], never a fabricated identity.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DnsLeakEchoRecord {
    /// The first `A` record's address. This is the original behaviour; a
    /// fake-IP resolver can poison it, so the shipped defaults prefer the TXT
    /// rules below.
    FirstAddress,
    /// The first character-string of the first `TXT` record, for authorities
    /// that answer with the resolver address as a single TXT value.
    TxtFirstValue,
    /// The first `TXT` character-string that is a valid IP address, for
    /// authorities that answer with the resolver address alongside unrelated
    /// TXT metadata (for example an EDNS client-subnet string). Zero address
    /// values, or more than one distinct address, is `InvalidResponse`.
    TxtFirstIpAddress,
    /// The character-string that follows the exact `key` string inside a
    /// `TXT` record, for authorities that answer `"key" "<value>"` pairs.
    TxtKeyedValue {
        /// The exact preceding character-string to match.
        key: String,
    },
}

impl DnsLeakEchoRecord {
    /// Whether the question for this rule is a `TXT` question.
    pub fn is_txt(&self) -> bool {
        !matches!(self, Self::FirstAddress)
    }
}

/// How the question name of a source is built.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DnsLeakProbeName {
    /// Prepend a fresh random label to the authority (defeats resolver
    /// caches, but needs an authority that answers wildcard subdomains).
    #[default]
    FreshSubdomain,
    /// Ask the authority name exactly. Fixed-name public echo services have no
    /// wildcard, so a generated label would only ever be NXDOMAIN.
    ExactAuthority,
}

/// One configured probe source: resolve `authority` through `resolver` and
/// report the identity the authority observed.
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
    /// The controlled echo authority zone (or the exact fixed name, when
    /// `probe_name` is [`DnsLeakProbeName::ExactAuthority`]).
    pub authority: String,
    /// The declared rule that reads this authority's answer.
    pub record: DnsLeakEchoRecord,
    /// How the question name is built for this authority.
    pub probe_name: DnsLeakProbeName,
}

impl DnsLeakProbeSource {
    /// A source that asks a fresh subdomain and reads the first `A` record.
    pub fn new(resolver: &str, authority: &str) -> Self {
        Self::with_record(resolver, authority, DnsLeakEchoRecord::FirstAddress)
    }

    /// A source that asks a fresh subdomain and reads the declared record.
    pub fn with_record(resolver: &str, authority: &str, record: DnsLeakEchoRecord) -> Self {
        Self {
            resolver: resolver.trim().to_owned(),
            authority: authority.trim().trim_end_matches('.').to_owned(),
            record,
            probe_name: DnsLeakProbeName::FreshSubdomain,
        }
    }

    /// A source that asks the authority name exactly (for fixed-name public
    /// echo services) and reads the declared record.
    pub fn exact(resolver: &str, authority: &str, record: DnsLeakEchoRecord) -> Self {
        Self {
            probe_name: DnsLeakProbeName::ExactAuthority,
            ..Self::with_record(resolver, authority, record)
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

/// Operation progress is separate from echo capability and last observed facts.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DnsLeakOperation {
    #[default]
    Idle,
    Running,
    Completed,
    Failed {
        failure: Failure,
    },
}

/// The last DNS leak cross-source probe of this host.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DnsLeakReport {
    #[serde(default)]
    pub operation: DnsLeakOperation,
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
            operation: DnsLeakOperation::Completed,
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

/// One source to echo-probe, carrying the already generated question.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DnsLeakEchoProbe {
    pub resolver: String,
    pub authority: String,
    /// The exact question asked of the authority.
    pub question: String,
    /// The declared extraction rule for the answer.
    pub record: DnsLeakEchoRecord,
}

impl DnsLeakEchoProbe {
    pub fn new(resolver: &str, authority: &str, question: &str, record: DnsLeakEchoRecord) -> Self {
        Self {
            resolver: resolver.to_owned(),
            authority: authority.to_owned(),
            question: question.to_owned(),
            record,
        }
    }
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
#[path = "dns_leak_test.rs"]
mod tests;
