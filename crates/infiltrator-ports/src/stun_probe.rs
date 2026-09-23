//! DUAL-14-09 (re-scoped): the host STUN UDP-egress capability and the shared
//! egress probe boundary.
//!
//! [`StunProbePort`] is the host capability that actually sends a STUN Binding
//! Request over UDP from this process and parses the XOR-MAPPED-ADDRESS the
//! server observed. [`StunEgressProbePort`] is the shared application boundary
//! both surfaces drive; it owns the configured server, the expected proxied
//! egress and the comparison, and publishes the one report the reader shows.
//!
//! A host without a STUN adapter injects nothing: both surfaces then publish
//! the typed unsupported state instead of a fabricated public mapping.

use async_trait::async_trait;
use infiltrator_contract::stun_probe::{StunProbeObservation, StunProbeReport, StunProbeRequest};

use crate::error::PortError;

#[async_trait]
pub trait StunProbePort: Send + Sync {
    /// Send one STUN Binding Request to the configured server and return the
    /// mapping it observed. Every network outcome (timeout, refusal, parse
    /// failure) is a typed result, not an error: the port only fails when it
    /// cannot run the probe at all.
    async fn observe(&self, request: StunProbeRequest) -> Result<StunProbeObservation, PortError>;
}

#[async_trait]
pub trait StunEgressProbePort: Send + Sync {
    /// Probe this host's UDP egress mapping through the configured STUN
    /// server and publish the typed comparison against the expected proxied
    /// egress. Fails only when this host has no STUN prober at all.
    async fn probe(&self) -> Result<StunProbeReport, PortError>;
}
