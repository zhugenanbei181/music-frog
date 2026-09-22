//! DUAL-14-10: the shared per-nameserver latency probe contract.
//!
//! The workbench measures the round trip to *each* configured nameserver
//! itself instead of reading a controller aggregate: the host encodes a
//! minimal DNS query, sends it over UDP (plain `ip:port` entries) or as a
//! wire-format DoH POST (`https://` entries), times the answer and validates
//! the response id before publishing a number. Everything both surfaces render
//! comes from this type, so neither surface can invent a latency.

use serde::{Deserialize, Serialize};

/// The question a speed test asks when the caller does not pick one.
pub const DEFAULT_PROBE_QUESTION: &str = "www.example.com";

/// Per-query deadline shared by both transports.
pub const DEFAULT_PROBE_TIMEOUT_MS: u32 = 2_000;

/// Highest latency a probe reports; a measurement above this is clamped so a
/// stalled answer can never overflow the shared read model.
pub const MAX_REPORTED_RTT_MS: u32 = 60_000;

/// Honest per-nameserver latency probe availability.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DnsLatencyStatus {
    /// A real per-nameserver prober is injected on this host.
    Ready,
    /// No host fact source reports a per-nameserver latency; the workbench
    /// never invents one.
    Unsupported { reason: String },
}

impl Default for DnsLatencyStatus {
    fn default() -> Self {
        Self::Unsupported {
            reason: "this host did not inject a DNS latency prober".to_owned(),
        }
    }
}

impl DnsLatencyStatus {
    pub fn unsupported(reason: &str) -> Self {
        Self::Unsupported {
            reason: reason.to_owned(),
        }
    }

    pub const fn is_ready(&self) -> bool {
        matches!(self, Self::Ready)
    }

    /// The typed refusal reason, when the host cannot measure.
    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::Ready => None,
            Self::Unsupported { reason } => Some(reason),
        }
    }
}

/// The transport one per-nameserver probe used.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DnsProbeTransport {
    /// Plain DNS over UDP.
    Udp,
    /// DNS over HTTPS (wire format) through the host HTTP client.
    Doh,
    /// This host cannot drive the address (DoT / DoQ / DNSCrypt / DHCP /
    /// `system`, or an HTTP/3 endpoint without an h3 client).
    Undrivable { reason: String },
}

/// What one nameserver answered.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DnsProbeOutcome {
    /// The host measured a validated answer.
    Measured { rtt_ms: u32 },
    /// No answer arrived before the deadline.
    TimedOut,
    /// An answer arrived but did not validate (wrong id, wrong question, not
    /// a response, or truncated).
    InvalidResponse { reason: String },
    /// The transport ran and failed (socket or HTTP error).
    Failed { message: String },
    /// The address was not probed at all.
    NotProbed { reason: String },
}

impl DnsProbeOutcome {
    /// The measured round trip, or `None` when nothing was measured.
    pub fn rtt_ms(&self) -> Option<u32> {
        match self {
            Self::Measured { rtt_ms } => Some(*rtt_ms),
            _ => None,
        }
    }

    pub const fn is_measured(&self) -> bool {
        matches!(self, Self::Measured { .. })
    }
}

/// One nameserver's last probe result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DnsServerLatency {
    pub address: String,
    pub is_fallback: bool,
    pub transport: DnsProbeTransport,
    pub outcome: DnsProbeOutcome,
}

/// A surface-facing summary of the last probe (both surfaces render this).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DnsLatencySummary {
    /// No probe has run in this session.
    NotProbed,
    /// The host has no prober.
    Unsupported { reason: String },
    /// Every configured nameserver answered.
    AllMeasured {
        count: usize,
        best_ms: u32,
        worst_ms: u32,
    },
    /// Some nameservers answered, some did not.
    Partial { measured: usize, total: usize },
    /// No nameserver answered within the deadline.
    NoneReachable { total: usize },
}

/// The last per-nameserver latency probe of this host.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DnsLatencyReport {
    pub status: DnsLatencyStatus,
    /// The question the host asked, so a result is never detached from it.
    pub question: String,
    pub results: Vec<DnsServerLatency>,
}

impl Default for DnsLatencyReport {
    fn default() -> Self {
        Self {
            status: DnsLatencyStatus::default(),
            question: DEFAULT_PROBE_QUESTION.to_owned(),
            results: Vec::new(),
        }
    }
}

impl DnsLatencyReport {
    /// A report for a host that cannot measure, carrying the typed reason.
    pub fn unsupported(reason: &str) -> Self {
        Self {
            status: DnsLatencyStatus::unsupported(reason),
            ..Self::default()
        }
    }

    /// A report built from a real probe.
    pub fn measured(question: &str, results: Vec<DnsServerLatency>) -> Self {
        Self {
            status: DnsLatencyStatus::Ready,
            question: question.to_owned(),
            results,
        }
    }

    /// Whether this report carries results from a host that can measure.
    pub fn is_probed(&self) -> bool {
        self.status.is_ready() && !self.results.is_empty()
    }

    /// The measured latency for one address, if it answered.
    pub fn latency_of(&self, address: &str) -> Option<u32> {
        self.results
            .iter()
            .find(|result| result.address == address)
            .and_then(|result| result.outcome.rtt_ms())
    }

    pub fn measured_count(&self) -> usize {
        self.results
            .iter()
            .filter(|result| result.outcome.is_measured())
            .count()
    }

    /// Every probed server that did not answer (never an empty-answer server:
    /// a server that was not attempted is `NotProbed` and is not reported as
    /// unreachable).
    pub fn unreachable(&self) -> Vec<&DnsServerLatency> {
        self.results
            .iter()
            .filter(|result| {
                matches!(
                    result.outcome,
                    DnsProbeOutcome::TimedOut
                        | DnsProbeOutcome::InvalidResponse { .. }
                        | DnsProbeOutcome::Failed { .. }
                )
            })
            .collect()
    }

    /// The real self-heal trigger DUAL-14-13 consumes: every attempted
    /// nameserver failed, so upstream resolution is genuinely unreachable.
    pub fn is_upstream_unreachable(&self) -> bool {
        self.is_probed() && self.measured_count() == 0 && !self.unreachable().is_empty()
    }

    fn measured_rtts(&self) -> Vec<u32> {
        self.results
            .iter()
            .filter_map(|result| result.outcome.rtt_ms())
            .collect()
    }

    /// The typed summary both surfaces localize.
    pub fn summary(&self) -> DnsLatencySummary {
        match &self.status {
            DnsLatencyStatus::Unsupported { reason } => DnsLatencySummary::Unsupported {
                reason: reason.clone(),
            },
            DnsLatencyStatus::Ready => {
                let measured = self.measured_count();
                if self.results.is_empty() {
                    DnsLatencySummary::NotProbed
                } else if measured == self.results.len() {
                    let rtts = self.measured_rtts();
                    DnsLatencySummary::AllMeasured {
                        count: measured,
                        best_ms: rtts.iter().copied().min().unwrap_or(0),
                        worst_ms: rtts.iter().copied().max().unwrap_or(0),
                    }
                } else if measured == 0 {
                    DnsLatencySummary::NoneReachable {
                        total: self.results.len(),
                    }
                } else {
                    DnsLatencySummary::Partial {
                        measured,
                        total: self.results.len(),
                    }
                }
            }
        }
    }
}

/// One nameserver to probe.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DnsProbeTarget {
    pub address: String,
    pub is_fallback: bool,
}

impl DnsProbeTarget {
    pub fn new(address: &str, is_fallback: bool) -> Self {
        Self {
            address: address.to_owned(),
            is_fallback,
        }
    }
}

/// The request handed to a host prober.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DnsLatencyProbeRequest {
    pub targets: Vec<DnsProbeTarget>,
    pub question: String,
    pub timeout_ms: u32,
}

impl DnsLatencyProbeRequest {
    pub fn new(targets: Vec<DnsProbeTarget>) -> Self {
        Self {
            targets,
            question: DEFAULT_PROBE_QUESTION.to_owned(),
            timeout_ms: DEFAULT_PROBE_TIMEOUT_MS,
        }
    }

    pub fn with_question(mut self, question: &str) -> Self {
        let question = question.trim().trim_end_matches('.');
        if !question.is_empty() {
            self.question = question.to_owned();
        }
        self
    }

    /// Clamp the deadline into a measurable range (0 would make every probe
    /// time out immediately, so it is never accepted).
    pub fn with_timeout_ms(mut self, timeout_ms: u32) -> Self {
        if timeout_ms > 0 {
            self.timeout_ms = timeout_ms.min(MAX_REPORTED_RTT_MS);
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn measured(address: &str, rtt_ms: u32) -> DnsServerLatency {
        DnsServerLatency {
            address: address.to_owned(),
            is_fallback: false,
            transport: DnsProbeTransport::Udp,
            outcome: DnsProbeOutcome::Measured { rtt_ms },
        }
    }

    fn timed_out(address: &str) -> DnsServerLatency {
        DnsServerLatency {
            address: address.to_owned(),
            is_fallback: false,
            transport: DnsProbeTransport::Udp,
            outcome: DnsProbeOutcome::TimedOut,
        }
    }

    #[test]
    fn a_default_report_never_claims_a_latency() {
        let report = DnsLatencyReport::default();
        assert!(!report.is_probed());
        assert_eq!(report.measured_count(), 0);
        assert!(report.unreachable().is_empty());
        assert!(!report.is_upstream_unreachable());
        assert_eq!(report.latency_of("223.5.5.5"), None);
        assert_eq!(
            report.summary(),
            DnsLatencySummary::Unsupported {
                reason: "this host did not inject a DNS latency prober".to_owned()
            }
        );
        let unsupported = DnsLatencyReport::unsupported("no prober on this host");
        assert!(!unsupported.status.is_ready());
        assert_eq!(unsupported.status.reason(), Some("no prober on this host"));
    }

    #[test]
    fn measured_and_unreachable_results_partition_a_real_probe() {
        let report = DnsLatencyReport::measured(
            DEFAULT_PROBE_QUESTION,
            vec![
                measured("223.5.5.5", 12),
                measured("1.1.1.1", 41),
                timed_out("8.8.8.8"),
            ],
        );
        assert!(report.is_probed());
        assert_eq!(report.question, "www.example.com");
        assert_eq!(report.latency_of("223.5.5.5"), Some(12));
        assert_eq!(report.latency_of("8.8.8.8"), None);
        assert_eq!(report.measured_count(), 2);
        assert_eq!(report.unreachable().len(), 1);
        assert!(!report.is_upstream_unreachable());
        assert_eq!(
            report.summary(),
            DnsLatencySummary::Partial {
                measured: 2,
                total: 3
            }
        );
    }

    #[test]
    fn an_all_measured_probe_reports_its_real_range() {
        let report = DnsLatencyReport::measured(
            DEFAULT_PROBE_QUESTION,
            vec![measured("223.5.5.5", 12), measured("1.1.1.1", 41)],
        );
        assert_eq!(
            report.summary(),
            DnsLatencySummary::AllMeasured {
                count: 2,
                best_ms: 12,
                worst_ms: 41
            }
        );
    }

    #[test]
    fn a_fully_unreachable_upstream_list_is_the_self_heal_trigger() {
        let report = DnsLatencyReport::measured(
            DEFAULT_PROBE_QUESTION,
            vec![
                timed_out("223.5.5.5"),
                DnsServerLatency {
                    address: "https://doh.pub/dns-query".to_owned(),
                    is_fallback: true,
                    transport: DnsProbeTransport::Doh,
                    outcome: DnsProbeOutcome::Failed {
                        message: "connection refused".to_owned(),
                    },
                },
            ],
        );
        assert!(report.is_upstream_unreachable());
        assert_eq!(
            report.summary(),
            DnsLatencySummary::NoneReachable { total: 2 }
        );
    }

    #[test]
    fn a_not_probed_address_is_not_reported_as_unreachable() {
        let report = DnsLatencyReport::measured(
            DEFAULT_PROBE_QUESTION,
            vec![
                measured("223.5.5.5", 12),
                DnsServerLatency {
                    address: "tls://1.0.0.1:853".to_owned(),
                    is_fallback: true,
                    transport: DnsProbeTransport::Undrivable {
                        reason: "DoT is not probed by this host".to_owned(),
                    },
                    outcome: DnsProbeOutcome::NotProbed {
                        reason: "DoT is not probed by this host".to_owned(),
                    },
                },
            ],
        );
        assert_eq!(report.unreachable().len(), 0);
        assert!(!report.is_upstream_unreachable());
        assert_eq!(
            report.summary(),
            DnsLatencySummary::Partial {
                measured: 1,
                total: 2
            }
        );
    }

    #[test]
    fn probe_requests_keep_the_shared_question_and_a_usable_deadline() {
        let request = DnsLatencyProbeRequest::new(vec![DnsProbeTarget::new("223.5.5.5", false)]);
        assert_eq!(request.question, DEFAULT_PROBE_QUESTION);
        assert_eq!(request.timeout_ms, DEFAULT_PROBE_TIMEOUT_MS);
        assert_eq!(request.targets[0].address, "223.5.5.5");

        let dotted = DnsLatencyProbeRequest::new(Vec::new()).with_question("  probe.example.  ");
        assert_eq!(dotted.question, "probe.example");
        // An empty question and a zero deadline never replace the defaults.
        let empty = DnsLatencyProbeRequest::new(Vec::new())
            .with_question("   ")
            .with_timeout_ms(0);
        assert_eq!(empty.question, DEFAULT_PROBE_QUESTION);
        assert_eq!(empty.timeout_ms, DEFAULT_PROBE_TIMEOUT_MS);
        assert_eq!(
            DnsLatencyProbeRequest::new(Vec::new())
                .with_timeout_ms(90_000)
                .timeout_ms,
            MAX_REPORTED_RTT_MS
        );
    }
}
