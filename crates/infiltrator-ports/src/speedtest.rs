//! Host-owned speedtest / jitter / packet-loss probe capability.
//!
//! The shared `SpeedtestApplication` engine lives in `infiltrator-application`;
//! this port is the zero-toolkit seam an inbound UI uses to trigger it and read
//! the canonical snapshot, without constructing a Mihomo client of its own.

use async_trait::async_trait;
use infiltrator_contract::capability::Capability;
use infiltrator_contract::speedtest::{SpeedtestScope, SpeedtestSnapshot};

use crate::error::PortError;

#[async_trait]
pub trait SpeedtestPort: Send + Sync {
    /// Current read model, including progress, node results and history.
    fn snapshot(&self) -> SpeedtestSnapshot;

    /// Run the concurrent latency probe over the requested scope, optionally
    /// overriding the target URL and per-probe timeout.
    async fn run_scope(
        &self,
        scope: SpeedtestScope,
        url: Option<String>,
        timeout_ms: Option<u32>,
    ) -> Result<SpeedtestSnapshot, PortError>;

    /// Probe delay + jitter + packet loss for a single node.
    async fn probe_node(
        &self,
        node: &str,
        rounds: usize,
        url: Option<String>,
        timeout_ms: Option<u32>,
    ) -> Result<SpeedtestSnapshot, PortError>;

    /// Record a measured downlink throughput sample for a node.
    ///
    /// The host performs the real transfer and reports the observed bytes and
    /// elapsed milliseconds; the shared engine converts them to Mbps and
    /// publishes the node in the canonical snapshot. No fabricated numbers.
    fn record_bandwidth(
        &self,
        node: &str,
        total_bytes: u64,
        duration_ms: u64,
    ) -> Result<f64, PortError>;

    /// DUAL-06-12: report the real egress IP + country the host observed when
    /// probing *through* the given node.
    ///
    /// The host performs the outbound probe (`probe_outbound_ip`) and reports
    /// the observed endpoint here; the shared engine stores the fact and never
    /// guesses one. The country may honestly be `None` when geolocation is
    /// unavailable.
    fn record_outbound_ip(
        &self,
        node: &str,
        ip: &str,
        country: Option<&str>,
    ) -> Result<(), PortError>;

    /// DUAL-06-12: probe the node's real egress IP + country.
    ///
    /// Hosts that cannot route a probe through a specific proxy node return a
    /// typed unsupported error instead of fabricating an endpoint. The default
    /// is the honest refusal; a real core adapter may override it.
    async fn probe_outbound_ip(&self, _node: &str) -> Result<(String, Option<String>), PortError> {
        Err(PortError::unsupported(
            Capability::Speedtest,
            "outbound ip probing through a node is not available on this host",
        ))
    }

    /// Request cancellation of the active batch. Returns whether one was running.
    fn cancel(&self) -> bool;

    /// DUAL-06-01: set the shared engine's concurrency limit at runtime.
    ///
    /// Hosts without a speedtest engine return a typed unsupported error
    /// instead of silently accepting a bound they cannot honor. The value is
    /// clamped to a minimum of 1 by the engine.
    fn set_concurrency(&self, _limit: usize) -> Result<(), PortError> {
        Err(PortError::unsupported(
            Capability::Speedtest,
            "speedtest engine is not available on this host",
        ))
    }
}
