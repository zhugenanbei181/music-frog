//! DUAL-14-10: host port for real per-nameserver latency measurement.
//!
//! The host owns the network I/O: it encodes a minimal DNS query, sends it to
//! each requested nameserver over UDP (`ip:port` entries) or as a wire-format
//! DoH POST (`https://` entries), times the answer and validates the response
//! id before reporting a number. Hosts without an adapter keep the empty
//! default and the workbench reports a typed unsupported status instead of a
//! fabricated latency.

use async_trait::async_trait;
use infiltrator_contract::dns_latency::{DnsLatencyProbeRequest, DnsLatencyReport};

use crate::error::PortError;

#[async_trait]
pub trait DnsLatencyProbePort: Send + Sync {
    /// Probe every requested nameserver and return the honest per-server
    /// report. A per-server failure is a result, not an error: the port only
    /// fails when it cannot run the probe at all.
    async fn probe(&self, request: DnsLatencyProbeRequest) -> Result<DnsLatencyReport, PortError>;
}
