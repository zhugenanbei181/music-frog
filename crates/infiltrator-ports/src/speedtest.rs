//! Host-owned speedtest / jitter / packet-loss probe capability.
//!
//! The shared `SpeedtestApplication` engine lives in `infiltrator-application`;
//! this port is the zero-toolkit seam an inbound UI uses to trigger it and read
//! the canonical snapshot, without constructing a Mihomo client of its own.

use async_trait::async_trait;
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

    /// Request cancellation of the active batch. Returns whether one was running.
    fn cancel(&self) -> bool;
}
