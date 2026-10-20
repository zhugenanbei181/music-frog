//! Engine-neutral detail facts observed for a single outbound.
//!
//! Every fact is modelled as a three-state observation: `Unknown` (not
//! observed yet), an observed value, and `Unsupported` (the backend cannot
//! report it at all). This keeps "not known yet" and "not available" apart,
//! so a surface never renders a missing capability as an empty success.

use super::identity::{
    OutboundCipherId, OutboundContractError, OutboundId, OutboundProtocolId, SnapshotRevision,
};
use crate::error::Failure;
use crate::identity::CoreScope;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashSet;

/// One outbound presented by an engine adapter.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutboundSnapshot {
    /// Stable identity of the outbound this snapshot describes.
    pub id: OutboundId,
    /// Missing display names remain unknown; consumers may render the opaque ID.
    pub display_name: Option<String>,
}

/// The evidence state for one independently observed outbound fact.
///
/// `Unknown` means the adapter has not observed a value. `Unsupported` means
/// the backend cannot provide this fact through its current adapter contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", content = "value", rename_all = "snake_case")]
pub enum OutboundObservation<T> {
    /// The adapter has not observed this fact yet.
    Unknown,
    /// The adapter observed this value.
    Observed(T),
    /// The backend cannot report this fact at all.
    Unsupported,
}

/// The network endpoint an outbound connects to.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutboundEndpoint {
    /// Host name or address of the endpoint.
    pub host: String,
    /// Port of the endpoint, when the backend reports one.
    pub port: OutboundObservation<u16>,
}

/// A transport feature an outbound may advertise.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutboundFeature {
    /// The outbound can carry UDP traffic.
    Udp,
}

/// The protocol an outbound uses, as reported by the backend.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", content = "value", rename_all = "snake_case")]
pub enum OutboundProtocolObservation {
    /// The protocol has not been observed yet.
    Unknown,
    /// The backend cannot report the protocol.
    Unsupported,
    /// The observed protocol, scoped to the same engine as the outbound.
    Reported(OutboundProtocolId),
}

/// The observed health of an outbound.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutboundHealthObservation {
    /// The health has not been observed yet.
    Unknown,
    /// The last probe reported the outbound as usable.
    Healthy,
    /// The last probe reported the outbound as unusable.
    Unhealthy,
    /// The backend cannot report health at all.
    Unsupported,
}

/// Whether an outbound supports UDP traffic.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutboundUdpSupport {
    /// UDP support has not been observed yet.
    Unknown,
    /// The backend reported that the outbound carries UDP traffic.
    Supported,
    /// The backend reported that the outbound cannot carry UDP traffic.
    Unsupported,
}

/// One latency measurement of an outbound.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutboundLatencySample {
    /// Human-readable timestamp of the measurement.
    pub observed_at: String,
    /// Measured round-trip delay in milliseconds.
    pub delay_ms: u32,
}

/// One observed egress address of an outbound.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutboundEgressSample {
    /// The observed egress IP address.
    pub ip: String,
    /// The country the egress address is attributed to, when known.
    pub country: Option<String>,
    /// Unix epoch milliseconds of the observation, when known.
    pub observed_at_epoch_ms: Option<u64>,
}

/// Aggregated round-trip-time statistics of an outbound.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutboundRttStatistics {
    /// Smallest observed round-trip time in milliseconds.
    pub min_ms: Option<u32>,
    /// Largest observed round-trip time in milliseconds.
    pub max_ms: Option<u32>,
    /// Mean observed round-trip time in milliseconds.
    pub avg_ms: Option<u32>,
    /// Number of samples the statistics were computed from.
    pub valid_count: usize,
}

/// The role an outbound plays inside a proxy group.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutboundCollectionKind {
    /// The user picks this member by hand.
    ManualSelection,
    /// The group selects this member automatically.
    AutomaticSelection,
    /// The group falls back to this member when the primary fails.
    Failover,
    /// The group spreads traffic across this member.
    LoadBalancing,
    /// The member relays traffic to another group.
    Relay,
}

/// Whether the backend can probe this outbound at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutboundProbeAvailability {
    /// The backend can probe the outbound.
    Supported,
    /// The backend cannot probe the outbound.
    Unsupported,
    /// The probe capability has not been determined yet.
    Unknown,
}

/// Typed, engine-neutral facts for one outbound. Adapters must only report
/// values supported by their observation source; absent wire fields stay
/// `Unknown` and non-existent backend fields stay `Unsupported`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutboundDetail {
    /// Stable identity of the described outbound.
    pub id: OutboundId,
    /// Protocol the outbound uses, when the backend reports it.
    pub protocol: OutboundProtocolObservation,
    /// Human-readable protocol name, when the backend reports one.
    pub protocol_display_name: Option<String>,
    /// Cipher the outbound uses, when the backend reports it.
    pub cipher: OutboundObservation<OutboundCipherId>,
    /// Human-readable cipher name, when the backend reports one.
    pub cipher_display_name: Option<String>,
    /// Role the outbound plays inside its group, when the backend reports it.
    pub collection_kind: OutboundObservation<OutboundCollectionKind>,
    /// Transport features the outbound advertises.
    pub features: OutboundObservation<Vec<OutboundFeature>>,
    /// Endpoint the outbound connects to, when the backend reports it.
    pub endpoint: OutboundObservation<OutboundEndpoint>,
    /// Whether the outbound can carry UDP traffic.
    pub udp: OutboundUdpSupport,
    /// Observed health of the outbound.
    pub health: OutboundHealthObservation,
    /// Most recently measured latency in milliseconds.
    pub current_latency_ms: OutboundObservation<u32>,
    /// Recorded latency history of the outbound.
    pub latency_history: OutboundObservation<Vec<OutboundLatencySample>>,
    /// Aggregated round-trip-time statistics of the outbound.
    pub rtt: OutboundObservation<OutboundRttStatistics>,
    /// Recorded egress addresses the outbound was seen using.
    pub historical_egress: OutboundObservation<Vec<OutboundEgressSample>>,
    /// Whether the backend is able to probe this outbound.
    pub probe_availability: OutboundProbeAvailability,
}

/// State of one outbound-details observation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutboundDetailsState {
    /// The details have not been observed yet.
    Unknown,
    /// The details were observed; the list is non-empty.
    Ready {
        /// The observed details, one per outbound, with unique IDs.
        details: Vec<OutboundDetail>,
    },
    /// The backend reported that no outbound exists.
    Empty,
    /// The backend cannot report outbound details at all.
    Unsupported,
    /// Reading the details failed.
    Failed {
        /// The typed reason the read failed.
        failure: Failure,
    },
}

/// Immutable details observation fenced to one active core and detail revision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OutboundDetailsSnapshot {
    scope: CoreScope,
    detail_revision: SnapshotRevision,
    state: OutboundDetailsState,
}

impl OutboundDetailsSnapshot {
    /// Builds a details snapshot and validates it against its scope.
    ///
    /// Fails with [`OutboundContractError::EmptyReadyDetails`] when a
    /// `Ready` state carries no details, with
    /// [`OutboundContractError::DuplicateOutboundDetailId`] when two details
    /// share an ID, and with [`OutboundContractError::EngineScopeMismatch`]
    /// when an outbound, protocol or cipher belongs to another engine than
    /// the fenced scope.
    pub fn new(
        scope: CoreScope,
        detail_revision: SnapshotRevision,
        state: OutboundDetailsState,
    ) -> Result<Self, OutboundContractError> {
        if let OutboundDetailsState::Ready { details } = &state {
            if details.is_empty() {
                return Err(OutboundContractError::EmptyReadyDetails);
            }
            let mut ids = HashSet::new();
            for detail in details {
                if detail.id.engine_id() != &scope.identity.engine {
                    return Err(OutboundContractError::EngineScopeMismatch);
                }
                if let OutboundProtocolObservation::Reported(protocol) = &detail.protocol
                    && protocol.engine_id() != &scope.identity.engine
                {
                    return Err(OutboundContractError::EngineScopeMismatch);
                }
                if let OutboundObservation::Observed(cipher) = &detail.cipher
                    && cipher.engine_id() != &scope.identity.engine
                {
                    return Err(OutboundContractError::EngineScopeMismatch);
                }
                if !ids.insert(&detail.id) {
                    return Err(OutboundContractError::DuplicateOutboundDetailId);
                }
            }
        }
        Ok(Self {
            scope,
            detail_revision,
            state,
        })
    }

    /// Returns the core scope this snapshot is fenced to.
    pub fn scope(&self) -> &CoreScope {
        &self.scope
    }

    /// Returns the detail revision this snapshot was observed at.
    pub const fn detail_revision(&self) -> SnapshotRevision {
        self.detail_revision
    }

    /// Returns the observed state.
    pub fn state(&self) -> &OutboundDetailsState {
        &self.state
    }

    /// Returns the detail of one outbound, when the snapshot is `Ready`.
    ///
    /// Returns `None` for every other state and for an unknown ID.
    pub fn detail(&self, id: &OutboundId) -> Option<&OutboundDetail> {
        let OutboundDetailsState::Ready { details } = &self.state else {
            return None;
        };
        details.iter().find(|detail| &detail.id == id)
    }
}

#[derive(Deserialize)]
struct OutboundDetailsSnapshotFields {
    scope: CoreScope,
    detail_revision: SnapshotRevision,
    state: OutboundDetailsState,
}

impl<'de> Deserialize<'de> for OutboundDetailsSnapshot {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let fields = OutboundDetailsSnapshotFields::deserialize(deserializer)?;
        Self::new(fields.scope, fields.detail_revision, fields.state).map_err(D::Error::custom)
    }
}
