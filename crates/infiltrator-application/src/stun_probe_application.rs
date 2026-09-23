//! DUAL-14-09 (re-scoped): the shared STUN UDP-egress probe use-case.
//!
//! One instance is shared by the command handler and the surface reader, so
//! the last real probe reaches both surfaces. The application owns the probe
//! policy: it sends one Binding Request to the configured STUN server through
//! the injected host prober and compares the observed mapping against the
//! expected proxied egress, if the host knows one.
//!
//! **Honest boundary.** The observation is *this host / this process's* UDP
//! egress mapping as seen by the STUN server. It is not a browser WebRTC
//! measurement and the application never publishes one. A host without a
//! prober stays a typed unsupported status; a host without an expected egress
//! publishes a mapping with an explicit `Unknown` comparison instead of an
//! implied "no leak".

use infiltrator_contract::stun_probe::{
    DEFAULT_STUN_SERVER, DEFAULT_STUN_TIMEOUT_MS, MAX_STUN_TIMEOUT_MS, StunMappedAddress,
    StunProbeReport, StunProbeRequest, StunProbeStatus,
};
use infiltrator_ports::error::PortError;
use infiltrator_ports::stun_probe::{StunEgressProbePort, StunProbePort};
use std::net::IpAddr;
use std::sync::{Arc, Mutex};

/// The typed reason a host without a STUN prober publishes.
pub const NO_STUN_PORT_REASON: &str = "this host injected no STUN UDP egress prober";

#[derive(Clone)]
pub struct StunProbeApplication {
    port: Option<Arc<dyn StunProbePort>>,
    server: String,
    timeout_ms: u32,
    expected_egress: Option<StunMappedAddress>,
    last: Arc<Mutex<StunProbeReport>>,
}

impl StunProbeApplication {
    pub fn new(port: Option<Arc<dyn StunProbePort>>, server: &str) -> Self {
        let server = server.trim();
        Self {
            port,
            server: if server.is_empty() {
                DEFAULT_STUN_SERVER.to_owned()
            } else {
                server.to_owned()
            },
            timeout_ms: DEFAULT_STUN_TIMEOUT_MS,
            expected_egress: None,
            last: Arc::new(Mutex::new(StunProbeReport::default())),
        }
    }

    /// A host that cannot probe keeps the honest typed refusal.
    pub fn unconfigured() -> Self {
        Self::new(None, DEFAULT_STUN_SERVER)
    }

    /// Probe with a caller-supplied deadline (used by the deterministic tests).
    pub fn with_timeout_ms(mut self, timeout_ms: u32) -> Self {
        if timeout_ms > 0 {
            self.timeout_ms = timeout_ms.min(MAX_STUN_TIMEOUT_MS);
        }
        self
    }

    /// Configure the public egress the host expects its traffic to use. Only a
    /// parseable IP is accepted; an unusable value leaves the comparison
    /// honestly `Unknown` instead of fabricating an expectation. The port of a
    /// configured expectation carries no meaning and is stored as `0`.
    pub fn with_expected_egress(mut self, ip: &str) -> Self {
        let ip = ip.trim();
        self.expected_egress = ip
            .parse::<IpAddr>()
            .ok()
            .map(|address| StunMappedAddress::new(address.to_string(), 0));
        self
    }

    pub fn has_prober(&self) -> bool {
        self.port.is_some()
    }

    pub fn server(&self) -> &str {
        &self.server
    }

    /// The typed status this host can currently publish.
    pub fn status(&self) -> StunProbeStatus {
        if self.port.is_none() {
            return StunProbeStatus::unsupported(NO_STUN_PORT_REASON);
        }
        self.last.lock().expect("STUN report lock").status.clone()
    }

    /// The last probe, with the server, transport, status and expected egress
    /// re-derived from the application so a host can never publish a stale
    /// readiness or a stale expectation.
    pub fn last_report(&self) -> StunProbeReport {
        let mut report = self.last.lock().expect("STUN report lock").clone();
        report.server = self.server.clone();
        report.expected_egress = self.expected_egress.clone();
        if self.port.is_none() {
            report.transport = infiltrator_contract::stun_probe::StunProbeTransport::Unsupported {
                reason: NO_STUN_PORT_REASON.to_owned(),
            };
            report.status = StunProbeStatus::unsupported(NO_STUN_PORT_REASON);
            report.mapping = None;
        }
        report
    }

    /// Ask the host prober once and publish the one shared report.
    async fn run_probe(&self, port: &Arc<dyn StunProbePort>) -> Result<StunProbeReport, PortError> {
        let request = StunProbeRequest::new(&self.server).with_timeout_ms(self.timeout_ms);
        let observation = port.observe(request).await?;
        let report = StunProbeReport::from_observation(observation, self.expected_egress.clone());
        *self.last.lock().expect("STUN report lock") = report.clone();
        Ok(report)
    }
}

#[async_trait::async_trait]
impl StunEgressProbePort for StunProbeApplication {
    async fn probe(&self) -> Result<StunProbeReport, PortError> {
        let Some(port) = self.port.as_ref() else {
            let report = StunProbeReport::unsupported(NO_STUN_PORT_REASON);
            *self.last.lock().expect("STUN report lock") = report;
            return Err(PortError::unsupported(
                infiltrator_contract::capability::Capability::Dns,
                NO_STUN_PORT_REASON,
            ));
        };
        self.run_probe(port).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::error::ErrorCode;
    use infiltrator_contract::stun_probe::{
        StunEgressComparison, StunProbeObservation, StunProbeTransport,
    };

    struct StubProbe {
        outcome: StunProbeObservation,
        seen: Mutex<Vec<StunProbeRequest>>,
    }

    impl StubProbe {
        fn new(outcome: StunProbeObservation) -> Arc<Self> {
            Arc::new(Self {
                outcome,
                seen: Mutex::new(Vec::new()),
            })
        }
    }

    #[async_trait::async_trait]
    impl StunProbePort for StubProbe {
        async fn observe(
            &self,
            request: StunProbeRequest,
        ) -> Result<StunProbeObservation, PortError> {
            self.seen.lock().expect("STUN request lock").push(request);
            Ok(self.outcome.clone())
        }
    }

    struct FailingProbe;

    #[async_trait::async_trait]
    impl StunProbePort for FailingProbe {
        async fn observe(
            &self,
            _request: StunProbeRequest,
        ) -> Result<StunProbeObservation, PortError> {
            Err(PortError::Failed("the STUN prober crashed".to_owned()))
        }
    }

    #[tokio::test]
    async fn a_host_without_a_stun_prober_reports_typed_unsupported_and_probes_nothing() {
        let application = StunProbeApplication::unconfigured();
        assert!(!application.has_prober());
        assert_eq!(application.status().reason(), Some(NO_STUN_PORT_REASON));
        let report = application.last_report();
        assert!(!report.is_observed());
        assert!(matches!(
            report.transport,
            StunProbeTransport::Unsupported { .. }
        ));
        assert!(matches!(
            report.comparison(),
            StunEgressComparison::Unknown { .. }
        ));

        let error = StunEgressProbePort::probe(&application)
            .await
            .expect_err("no prober must refuse");
        assert_eq!(error.error_code(), ErrorCode::Unsupported);
    }

    #[tokio::test]
    async fn an_observed_mapping_and_expected_egress_are_consistent() {
        let probe = StubProbe::new(StunProbeObservation::observed(
            DEFAULT_STUN_SERVER,
            StunMappedAddress::new("203.0.113.9", 51234),
        ));
        let application = StunProbeApplication::new(Some(probe.clone()), DEFAULT_STUN_SERVER)
            .with_expected_egress("203.0.113.9");

        let report = StunEgressProbePort::probe(&application)
            .await
            .expect("probe");
        assert!(report.is_observed());
        assert!(report.comparison().is_consistent());
        assert_eq!(application.last_report(), report);

        let seen = probe.seen.lock().expect("STUN request lock");
        assert_eq!(seen[0].server, DEFAULT_STUN_SERVER);
        assert_eq!(seen[0].timeout_ms, DEFAULT_STUN_TIMEOUT_MS);
    }

    #[tokio::test]
    async fn a_differing_mapping_is_a_divergent_fact_not_a_leak_verdict() {
        let probe = StubProbe::new(StunProbeObservation::observed(
            DEFAULT_STUN_SERVER,
            StunMappedAddress::new("198.51.100.7", 51234),
        ));
        let application = StunProbeApplication::new(Some(probe), DEFAULT_STUN_SERVER)
            .with_expected_egress("203.0.113.9");
        let report = StunEgressProbePort::probe(&application)
            .await
            .expect("probe");
        assert!(report.comparison().is_divergent());
        assert_eq!(report.comparison().reason(), None);
        assert_eq!(
            report.mapping().map(|mapping| mapping.ip.as_str()),
            Some("198.51.100.7")
        );
    }

    #[tokio::test]
    async fn a_host_without_an_expected_egress_publishes_an_unknown_comparison() {
        let probe = StubProbe::new(StunProbeObservation::observed(
            DEFAULT_STUN_SERVER,
            StunMappedAddress::new("203.0.113.9", 51234),
        ));
        let application = StunProbeApplication::new(Some(probe), DEFAULT_STUN_SERVER);
        let report = StunEgressProbePort::probe(&application)
            .await
            .expect("probe");
        assert!(report.is_observed());
        assert_eq!(report.expected_egress_ip(), None);
        assert!(matches!(
            report.comparison(),
            StunEgressComparison::Unknown { .. }
        ));
    }

    #[tokio::test]
    async fn a_timed_out_probe_keeps_the_typed_status_and_no_mapping() {
        let probe = StubProbe::new(StunProbeObservation::timed_out(DEFAULT_STUN_SERVER));
        let application = StunProbeApplication::new(Some(probe), DEFAULT_STUN_SERVER)
            .with_expected_egress("203.0.113.9");
        let report = StunEgressProbePort::probe(&application)
            .await
            .expect("probe");
        assert_eq!(report.status, StunProbeStatus::TimedOut);
        assert!(report.mapping().is_none());
        assert!(matches!(
            report.comparison(),
            StunEgressComparison::Unknown { .. }
        ));
    }

    #[tokio::test]
    async fn a_prober_error_never_overwrites_the_last_honest_report() {
        let application =
            StunProbeApplication::new(Some(Arc::new(FailingProbe)), DEFAULT_STUN_SERVER);
        let error = StunEgressProbePort::probe(&application)
            .await
            .expect_err("the host prober failed");
        assert_eq!(error.error_code(), ErrorCode::Internal);
        let report = application.last_report();
        assert!(!report.is_observed());
        assert_eq!(report.status, StunProbeStatus::Unknown);
    }

    #[test]
    fn the_default_server_is_the_real_public_stun_server() {
        let application = StunProbeApplication::unconfigured();
        assert_eq!(application.server(), "stun.l.google.com:19302");
        assert_eq!(DEFAULT_STUN_SERVER, "stun.l.google.com:19302");
    }

    #[test]
    fn an_unusable_expected_egress_stays_unknown_instead_of_fabricating_one() {
        let application = StunProbeApplication::unconfigured().with_expected_egress("not-an-ip");
        assert!(application.last_report().expected_egress.is_none());

        let configured =
            StunProbeApplication::unconfigured().with_expected_egress("  203.0.113.9  ");
        assert_eq!(
            configured.last_report().expected_egress_ip(),
            Some("203.0.113.9")
        );

        let blank = StunProbeApplication::unconfigured().with_expected_egress("");
        assert!(blank.last_report().expected_egress.is_none());
    }
}
