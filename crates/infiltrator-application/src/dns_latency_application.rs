//! DUAL-14-10: the shared per-nameserver latency probe use-case.
//!
//! One instance is shared by the command handler and the surface reader, so
//! the last real probe reaches both surfaces. The application owns the probe
//! policy (question, deadline, which configured nameservers are probed); the
//! host port owns the network I/O. A host without a port keeps the typed
//! unsupported status and the command answers with a typed refusal instead of
//! a fabricated number.

use infiltrator_contract::dns_latency::{
    DnsLatencyProbeRequest, DnsLatencyReport, DnsLatencyStatus, DnsProbeTarget, MAX_REPORTED_RTT_MS,
};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::surface_snapshot::DnsServerSnapshot;
use infiltrator_ports::dns_latency::DnsLatencyProbePort;
use infiltrator_ports::error::PortError;
use std::sync::{Arc, Mutex};

/// The typed reason a host without a prober publishes.
pub const NO_PROBER_REASON: &str = "this host injected no DNS latency prober";

#[derive(Clone)]
pub struct DnsLatencyApplication {
    port: Option<Arc<dyn DnsLatencyProbePort>>,
    last: Arc<Mutex<DnsLatencyReport>>,
}

impl DnsLatencyApplication {
    pub fn new(port: Option<Arc<dyn DnsLatencyProbePort>>) -> Self {
        Self {
            port,
            last: Arc::new(Mutex::new(DnsLatencyReport::unsupported(NO_PROBER_REASON))),
        }
    }

    /// A host that cannot measure keeps the honest typed refusal.
    pub fn unconfigured() -> Self {
        Self::new(None)
    }

    pub fn has_prober(&self) -> bool {
        self.port.is_some()
    }

    /// Whether this host can produce real per-nameserver latency facts.
    pub fn status(&self) -> DnsLatencyStatus {
        match self.port {
            Some(_) => DnsLatencyStatus::Ready,
            None => DnsLatencyStatus::unsupported(NO_PROBER_REASON),
        }
    }

    /// The last probe, with the status re-derived from the injected port so a
    /// host can never publish a stale readiness.
    pub fn last_report(&self) -> DnsLatencyReport {
        let mut report = self.last.lock().expect("dns latency report lock").clone();
        report.status = self.status();
        report
    }

    /// Probe every configured nameserver through the injected host prober.
    pub async fn probe(&self, servers: &[DnsServerSnapshot]) -> Result<DnsLatencyReport, Failure> {
        if servers.is_empty() {
            return Err(Failure::new(
                ErrorCode::InvalidInput,
                "the profile configures no DNS nameserver to probe",
                false,
            ));
        }
        let request = DnsLatencyProbeRequest::new(targets_from_servers(servers));
        DnsLatencyProbePort::probe(self, request)
            .await
            .map_err(Failure::from)
    }
}

/// The application is itself the host port surfaces call, so a probe started
/// from either surface lands in the one shared `last` report the reader
/// publishes. This mirrors `RuleTracerApplication` implementing `RuleTracerPort`.
#[async_trait::async_trait]
impl DnsLatencyProbePort for DnsLatencyApplication {
    async fn probe(&self, request: DnsLatencyProbeRequest) -> Result<DnsLatencyReport, PortError> {
        let Some(port) = self.port.as_ref() else {
            let report = DnsLatencyReport::unsupported(NO_PROBER_REASON);
            *self.last.lock().expect("dns latency report lock") = report;
            return Err(PortError::unsupported(
                infiltrator_contract::capability::Capability::Dns,
                NO_PROBER_REASON,
            ));
        };
        let timeout_ms = request.timeout_ms.min(MAX_REPORTED_RTT_MS);
        let request = request.with_timeout_ms(timeout_ms);
        let report = port.probe(request).await?;
        *self.last.lock().expect("dns latency report lock") = report.clone();
        Ok(report)
    }
}

/// The configured nameservers the prober must attempt, in profile order.
pub fn targets_from_servers(servers: &[DnsServerSnapshot]) -> Vec<DnsProbeTarget> {
    servers
        .iter()
        .map(|server| DnsProbeTarget::new(&server.address, server.is_fallback))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use infiltrator_contract::dns_latency::{
        DEFAULT_PROBE_QUESTION, DnsLatencySummary, DnsProbeOutcome, DnsProbeTransport,
        DnsServerLatency,
    };
    use infiltrator_ports::error::PortError;

    struct RecordingProber {
        seen: Mutex<Option<DnsLatencyProbeRequest>>,
        fail: bool,
    }

    #[async_trait]
    impl DnsLatencyProbePort for RecordingProber {
        async fn probe(
            &self,
            request: DnsLatencyProbeRequest,
        ) -> Result<DnsLatencyReport, PortError> {
            let question = request.question.clone();
            let results = request
                .targets
                .iter()
                .enumerate()
                .map(|(index, target)| DnsServerLatency {
                    address: target.address.clone(),
                    is_fallback: target.is_fallback,
                    transport: DnsProbeTransport::Udp,
                    outcome: if self.fail {
                        DnsProbeOutcome::TimedOut
                    } else {
                        DnsProbeOutcome::Measured {
                            rtt_ms: 10 + index as u32,
                        }
                    },
                })
                .collect();
            *self.seen.lock().expect("probe request lock") = Some(request);
            Ok(DnsLatencyReport::measured(&question, results))
        }
    }

    fn server(address: &str, is_fallback: bool) -> DnsServerSnapshot {
        DnsServerSnapshot {
            address: address.to_owned(),
            protocol: "UDP".to_owned(),
            latency_ms: None,
            is_fallback,
            tags: Vec::new(),
        }
    }

    #[tokio::test]
    async fn a_host_without_a_prober_reports_typed_unsupported_and_probes_nothing() {
        let application = DnsLatencyApplication::unconfigured();
        assert!(!application.has_prober());
        assert!(!application.status().is_ready());
        assert_eq!(application.status().reason(), Some(NO_PROBER_REASON));
        assert!(!application.last_report().is_probed());
        assert_eq!(
            application.last_report().summary(),
            DnsLatencySummary::Unsupported {
                reason: NO_PROBER_REASON.to_owned()
            }
        );

        let error = application
            .probe(&[server("223.5.5.5", false)])
            .await
            .expect_err("no prober must refuse");
        assert_eq!(error.code, ErrorCode::Unsupported);
    }

    #[tokio::test]
    async fn a_real_probe_publishes_the_measured_results_to_both_readers() {
        let prober = Arc::new(RecordingProber {
            seen: Mutex::new(None),
            fail: false,
        });
        let application = DnsLatencyApplication::new(Some(prober.clone()));
        assert!(application.has_prober());
        assert!(application.status().is_ready());

        let report = application
            .probe(&[
                server("223.5.5.5", false),
                server("https://doh.pub/dns-query", true),
            ])
            .await
            .expect("probe");

        let seen = prober.seen.lock().expect("probe request lock").clone();
        let seen = seen.expect("the prober received the request");
        assert_eq!(seen.question, DEFAULT_PROBE_QUESTION);
        assert_eq!(seen.targets.len(), 2);
        assert!(seen.targets[1].is_fallback);
        assert_eq!(report.latency_of("223.5.5.5"), Some(10));
        assert_eq!(report.latency_of("https://doh.pub/dns-query"), Some(11));
        // The reader sees exactly the same report.
        assert_eq!(application.last_report(), report);
    }

    #[tokio::test]
    async fn an_unreachable_upstream_list_is_published_not_hidden() {
        let application = DnsLatencyApplication::new(Some(Arc::new(RecordingProber {
            seen: Mutex::new(None),
            fail: true,
        })));
        let report = application
            .probe(&[server("223.5.5.5", false), server("1.1.1.1", false)])
            .await
            .expect("probe");
        assert!(report.is_upstream_unreachable());
        assert_eq!(report.measured_count(), 0);
        assert_eq!(report.unreachable().len(), 2);
    }

    #[tokio::test]
    async fn probing_without_a_configured_nameserver_is_a_typed_input_error() {
        let application = DnsLatencyApplication::new(Some(Arc::new(RecordingProber {
            seen: Mutex::new(None),
            fail: false,
        })));
        let error = application.probe(&[]).await.expect_err("no targets");
        assert_eq!(error.code, ErrorCode::InvalidInput);
        // A refused probe never overwrites the last honest report.
        assert!(!application.last_report().is_probed());
    }

    #[test]
    fn targets_mirror_the_projected_server_list() {
        let targets = targets_from_servers(&[
            server("223.5.5.5", false),
            server("tls://1.0.0.1:853", true),
        ]);
        assert_eq!(targets.len(), 2);
        assert_eq!(targets[0].address, "223.5.5.5");
        assert!(!targets[0].is_fallback);
        assert!(targets[1].is_fallback);
    }
}
