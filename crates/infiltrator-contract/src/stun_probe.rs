//! DUAL-14-09 (re-scoped): the shared STUN UDP-egress probe contract.
//!
//! **What this measures (the honest re-scope).** An external STUN server
//! answers a Binding Request sent by *this host / this process* over UDP, so
//! the observation is the host's own UDP egress mapping (public IP + port) as
//! seen by that server. It is **not** a browser WebRTC measurement: no browser
//! stack is involved, and a single host-process STUN exchange cannot observe
//! how a browser's ICE agent would behave. The report therefore never claims a
//! "WebRTC leak".
//!
//! The report also carries an optional *expected proxied egress* (the public
//! egress the host expects its traffic to use) and derives a comparison
//! ([`StunEgressComparison`]) without inventing a verdict: when either side is
//! unknown the comparison is [`StunEgressComparison::Unknown`], and a
//! differing IP is reported as diverging facts, not as a leak.
//!
//! Everything both surfaces render comes from these types.

use serde::{Deserialize, Serialize};

/// The configured default STUN server used when the host does not override it.
pub const DEFAULT_STUN_SERVER: &str = "stun.l.google.com:19302";

/// The STUN Binding Request default deadline.
pub const DEFAULT_STUN_TIMEOUT_MS: u32 = 3_000;

/// Highest deadline a STUN probe accepts; a larger request is clamped so a
/// stalled server can never widen the shared probe window.
pub const MAX_STUN_TIMEOUT_MS: u32 = 60_000;

/// The reason a comparison is unknown because nothing has been observed yet.
pub const NO_OBSERVED_MAPPING_REASON: &str =
    "no UDP egress mapping has been observed from a STUN server yet";

/// The reason a comparison is unknown because the host knows no expected
/// proxied egress. A mismatch is never inferred from a missing expectation.
pub const NO_EXPECTED_EGRESS_REASON: &str = "no expected proxied egress is known on this host";

/// The public transport address a STUN server observed for this host's UDP
/// flow. Only the IP takes part in the egress comparison: the port is the
/// ephemeral source port of the probe socket and has no stable meaning.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StunMappedAddress {
    pub ip: String,
    pub port: u16,
}

impl StunMappedAddress {
    pub fn new(ip: impl Into<String>, port: u16) -> Self {
        Self {
            ip: ip.into(),
            port,
        }
    }

    /// The `ip` or `[ip]:port` display form, with an IPv6 address bracketed.
    pub fn display(&self) -> String {
        if self.ip.contains(':') {
            format!("[{}]:{}", self.ip, self.port)
        } else {
            format!("{}:{}", self.ip, self.port)
        }
    }
}

/// The transport one observation used. Only UDP can be driven by this host;
/// an unimplemented transport is a typed refusal, never a fabricated mapping.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum StunProbeTransport {
    /// A plain STUN Binding Request over UDP.
    #[default]
    Udp,
    /// This host cannot drive the transport (for example a TLS/TCP-only STUN
    /// endpoint); no binding request was sent.
    Unsupported { reason: String },
}

/// The typed outcome of one STUN probe exchange.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum StunProbeStatus {
    /// Nothing has been reported yet: the host neither confirmed a prober nor
    /// refused one, so this must stay a non-result.
    #[default]
    Unknown,
    /// A Binding Success Response carried a XOR-MAPPED-ADDRESS.
    Observed,
    /// No Binding Response arrived before the deadline.
    TimedOut,
    /// The transport ran and failed (socket, DNS or send/recv error).
    Failed { message: String },
    /// No host STUN prober is composed; no request was sent.
    Unsupported { reason: String },
}

impl StunProbeStatus {
    pub fn unsupported(reason: &str) -> Self {
        Self::Unsupported {
            reason: reason.to_owned(),
        }
    }

    pub const fn is_observed(&self) -> bool {
        matches!(self, Self::Observed)
    }

    /// The typed refusal reason, when the host cannot probe.
    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::Unsupported { reason } => Some(reason),
            _ => None,
        }
    }

    /// The typed failure message, when the transport ran and failed.
    pub fn failure(&self) -> Option<&str> {
        match self {
            Self::Failed { message } => Some(message),
            _ => None,
        }
    }
}

/// How the observed UDP egress mapping compares against the expected proxied
/// egress. This is a *fact comparison*: it never labels one side a leak.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StunEgressComparison {
    /// The observed UDP egress IP equals the expected proxied egress IP.
    Consistent,
    /// The observed UDP egress IP differs from the expected proxied egress IP.
    /// Both facts are published; the app does not decide which is correct.
    Divergent,
    /// Nothing can be compared yet: no observation, or no expected egress.
    Unknown { reason: String },
}

impl StunEgressComparison {
    pub const fn is_consistent(&self) -> bool {
        matches!(self, Self::Consistent)
    }

    pub const fn is_divergent(&self) -> bool {
        matches!(self, Self::Divergent)
    }

    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::Unknown { reason } => Some(reason),
            _ => None,
        }
    }
}

/// One STUN probe exchange: the server asked, the transport, the typed status
/// and the mapping the server observed (present only when `status` is
/// [`StunProbeStatus::Observed`]).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StunProbeObservation {
    pub server: String,
    pub transport: StunProbeTransport,
    pub status: StunProbeStatus,
    pub mapping: Option<StunMappedAddress>,
}

impl StunProbeObservation {
    /// A success with the address the STUN server observed.
    pub fn observed(server: &str, mapping: StunMappedAddress) -> Self {
        Self {
            server: server.to_owned(),
            transport: StunProbeTransport::Udp,
            status: StunProbeStatus::Observed,
            mapping: Some(mapping),
        }
    }

    /// No Binding Response arrived before the deadline.
    pub fn timed_out(server: &str) -> Self {
        Self {
            server: server.to_owned(),
            transport: StunProbeTransport::Udp,
            status: StunProbeStatus::TimedOut,
            mapping: None,
        }
    }

    /// The transport ran and failed.
    pub fn failed(server: &str, message: impl Into<String>) -> Self {
        Self {
            server: server.to_owned(),
            transport: StunProbeTransport::Udp,
            status: StunProbeStatus::Failed {
                message: message.into(),
            },
            mapping: None,
        }
    }

    /// This host composed no STUN prober.
    pub fn unsupported(reason: &str) -> Self {
        Self {
            server: String::new(),
            transport: StunProbeTransport::Unsupported {
                reason: reason.to_owned(),
            },
            status: StunProbeStatus::unsupported(reason),
            mapping: None,
        }
    }
}

/// The last STUN UDP-egress observation of this host, plus the optional
/// expected proxied egress it is compared against.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StunProbeReport {
    pub server: String,
    pub transport: StunProbeTransport,
    pub status: StunProbeStatus,
    pub mapping: Option<StunMappedAddress>,
    /// The public egress the host expects its traffic to use, when known.
    pub expected_egress: Option<StunMappedAddress>,
}

impl StunProbeReport {
    /// A report for a host that composed no STUN prober.
    pub fn unsupported(reason: &str) -> Self {
        let observation = StunProbeObservation::unsupported(reason);
        Self {
            server: observation.server,
            transport: observation.transport,
            status: observation.status,
            mapping: observation.mapping,
            expected_egress: None,
        }
    }

    /// A report built from one host observation and the expected egress, when
    /// the application knows one.
    pub fn from_observation(
        observation: StunProbeObservation,
        expected_egress: Option<StunMappedAddress>,
    ) -> Self {
        Self {
            server: observation.server,
            transport: observation.transport,
            status: observation.status,
            mapping: observation.mapping,
            expected_egress,
        }
    }

    pub const fn is_observed(&self) -> bool {
        self.status.is_observed() && self.mapping.is_some()
    }

    /// Whether the host has actually reported an outcome (observed, timed out,
    /// failed or unsupported) rather than the initial not-yet-run state.
    pub const fn is_reported(&self) -> bool {
        !matches!(self.status, StunProbeStatus::Unknown)
    }

    pub fn mapping(&self) -> Option<&StunMappedAddress> {
        self.mapping.as_ref()
    }

    /// The expected egress IP, when the host knows one.
    pub fn expected_egress_ip(&self) -> Option<&str> {
        self.expected_egress
            .as_ref()
            .map(|egress| egress.ip.as_str())
    }

    /// How the observed mapping compares against the expected proxied egress.
    ///
    /// The derivation is deliberately conservative: an unfinished or failed
    /// probe, or a host with no expected egress, is [`Unknown`] rather than an
    /// implied "no leak". Only two concrete IPs are ever compared.
    ///
    /// [`Unknown`]: StunEgressComparison::Unknown
    pub fn comparison(&self) -> StunEgressComparison {
        let Some(mapping) = self.mapping.as_ref().filter(|_| self.status.is_observed()) else {
            return StunEgressComparison::Unknown {
                reason: NO_OBSERVED_MAPPING_REASON.to_owned(),
            };
        };
        let Some(expected) = self.expected_egress.as_ref() else {
            return StunEgressComparison::Unknown {
                reason: NO_EXPECTED_EGRESS_REASON.to_owned(),
            };
        };
        if mapping.ip.trim().eq_ignore_ascii_case(expected.ip.trim()) {
            StunEgressComparison::Consistent
        } else {
            StunEgressComparison::Divergent
        }
    }
}

/// One STUN Binding Request to send to `server`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StunProbeRequest {
    pub server: String,
    pub timeout_ms: u32,
}

impl StunProbeRequest {
    pub fn new(server: &str) -> Self {
        Self {
            server: server.trim().to_owned(),
            timeout_ms: DEFAULT_STUN_TIMEOUT_MS,
        }
    }

    /// Clamp the deadline into a usable range (0 would make every probe time
    /// out immediately, so it is never accepted).
    pub fn with_timeout_ms(mut self, timeout_ms: u32) -> Self {
        if timeout_ms > 0 {
            self.timeout_ms = timeout_ms.min(MAX_STUN_TIMEOUT_MS);
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expected(ip: &str) -> Option<StunMappedAddress> {
        Some(StunMappedAddress::new(ip, 443))
    }

    #[test]
    fn a_default_report_is_unknown_and_never_a_verdict() {
        let report = StunProbeReport::default();
        assert!(!report.is_observed());
        assert_eq!(report.status, StunProbeStatus::Unknown);
        assert_eq!(report.mapping(), None);
        assert_eq!(
            report.comparison(),
            StunEgressComparison::Unknown {
                reason: NO_OBSERVED_MAPPING_REASON.to_owned()
            }
        );
        assert!(!report.comparison().is_consistent());
        assert!(!report.comparison().is_divergent());
    }

    #[test]
    fn an_observed_mapping_without_an_expected_egress_is_unknown() {
        let report = StunProbeReport::from_observation(
            StunProbeObservation::observed(
                "stun.example.org:3478",
                StunMappedAddress::new("203.0.113.9", 51234),
            ),
            None,
        );
        assert!(report.is_observed());
        assert_eq!(report.expected_egress_ip(), None);
        assert_eq!(
            report.comparison(),
            StunEgressComparison::Unknown {
                reason: NO_EXPECTED_EGRESS_REASON.to_owned()
            }
        );
    }

    #[test]
    fn matching_ips_are_consistent_and_the_port_is_ignored() {
        let report = StunProbeReport::from_observation(
            StunProbeObservation::observed(
                "stun.example.org:3478",
                StunMappedAddress::new("203.0.113.9", 51234),
            ),
            expected("203.0.113.9"),
        );
        assert!(report.comparison().is_consistent());
        assert_eq!(report.comparison().reason(), None);
    }

    #[test]
    fn differing_ips_are_divergent_facts_not_a_leak_verdict() {
        let report = StunProbeReport::from_observation(
            StunProbeObservation::observed(
                "stun.example.org:3478",
                StunMappedAddress::new("198.51.100.7", 51234),
            ),
            expected("203.0.113.9"),
        );
        let comparison = report.comparison();
        assert!(comparison.is_divergent());
        assert_eq!(comparison.reason(), None);
        assert!(!comparison.is_consistent());
    }

    #[test]
    fn a_failed_or_timed_out_probe_never_compares() {
        for observation in [
            StunProbeObservation::timed_out("stun.example.org:3478"),
            StunProbeObservation::failed("stun.example.org:3478", "network unreachable"),
        ] {
            let report = StunProbeReport::from_observation(observation, expected("203.0.113.9"));
            assert!(!report.is_observed());
            assert_eq!(
                report.comparison(),
                StunEgressComparison::Unknown {
                    reason: NO_OBSERVED_MAPPING_REASON.to_owned()
                }
            );
        }
    }

    #[test]
    fn an_unsupported_host_is_a_typed_refusal_not_a_result() {
        let report = StunProbeReport::unsupported("no STUN prober is composed");
        assert!(!report.is_observed());
        assert_eq!(report.status.reason(), Some("no STUN prober is composed"));
        assert_eq!(report.mapping(), None);
        assert!(matches!(
            report.transport,
            StunProbeTransport::Unsupported { .. }
        ));
        assert!(matches!(
            report.comparison(),
            StunEgressComparison::Unknown { .. }
        ));
    }

    #[test]
    fn mapped_addresses_bracket_ipv6_and_round_trip_through_serde() {
        assert_eq!(
            StunMappedAddress::new("203.0.113.9", 51234).display(),
            "203.0.113.9:51234"
        );
        assert_eq!(
            StunMappedAddress::new("2001:db8::1", 3478).display(),
            "[2001:db8::1]:3478"
        );

        let report = StunProbeReport::from_observation(
            StunProbeObservation::observed(
                "stun.example.org:3478",
                StunMappedAddress::new("203.0.113.9", 51234),
            ),
            expected("203.0.113.9"),
        );
        let encoded = serde_json::to_string(&report).expect("serialize");
        let decoded: StunProbeReport = serde_json::from_str(&encoded).expect("deserialize");
        assert_eq!(decoded, report);
        assert!(decoded.comparison().is_consistent());
    }

    #[test]
    fn requests_keep_a_usable_deadline_and_the_server() {
        let request = StunProbeRequest::new("  stun.example.org:3478  ");
        assert_eq!(request.server, "stun.example.org:3478");
        assert_eq!(request.timeout_ms, DEFAULT_STUN_TIMEOUT_MS);
        assert_eq!(
            StunProbeRequest::new(DEFAULT_STUN_SERVER)
                .with_timeout_ms(0)
                .timeout_ms,
            DEFAULT_STUN_TIMEOUT_MS
        );
        assert_eq!(
            StunProbeRequest::new(DEFAULT_STUN_SERVER)
                .with_timeout_ms(900_000)
                .timeout_ms,
            MAX_STUN_TIMEOUT_MS
        );
    }

    #[test]
    fn out_of_range_ports_and_bad_servers_are_still_just_data() {
        // The contract stores what the host observed; it does not validate a
        // network fact into existence or out of existence.
        let report = StunProbeReport::from_observation(
            StunProbeObservation::failed("not-a-host", "cannot resolve"),
            None,
        );
        assert_eq!(report.status.failure(), Some("cannot resolve"));
        assert!(report.mapping().is_none());
    }
}
