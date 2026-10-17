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
    Unknown,
    Observed(T),
    Unsupported,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutboundEndpoint {
    pub host: String,
    pub port: OutboundObservation<u16>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutboundFeature {
    Udp,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", content = "value", rename_all = "snake_case")]
pub enum OutboundProtocolObservation {
    Unknown,
    Unsupported,
    Reported(OutboundProtocolId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutboundHealthObservation {
    Unknown,
    Healthy,
    Unhealthy,
    Unsupported,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutboundUdpSupport {
    Unknown,
    Supported,
    Unsupported,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutboundLatencySample {
    pub observed_at: String,
    pub delay_ms: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutboundEgressSample {
    pub ip: String,
    pub country: Option<String>,
    pub observed_at_epoch_ms: Option<u64>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutboundRttStatistics {
    pub min_ms: Option<u32>,
    pub max_ms: Option<u32>,
    pub avg_ms: Option<u32>,
    pub valid_count: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutboundCollectionKind {
    ManualSelection,
    AutomaticSelection,
    Failover,
    LoadBalancing,
    Relay,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutboundProbeAvailability {
    Supported,
    Unsupported,
    Unknown,
}

/// Typed, engine-neutral facts for one outbound. Adapters must only report
/// values supported by their observation source; absent wire fields stay
/// `Unknown` and non-existent backend fields stay `Unsupported`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutboundDetail {
    pub id: OutboundId,
    pub protocol: OutboundProtocolObservation,
    pub protocol_display_name: Option<String>,
    pub cipher: OutboundObservation<OutboundCipherId>,
    pub cipher_display_name: Option<String>,
    pub collection_kind: OutboundObservation<OutboundCollectionKind>,
    pub features: OutboundObservation<Vec<OutboundFeature>>,
    pub endpoint: OutboundObservation<OutboundEndpoint>,
    pub udp: OutboundUdpSupport,
    pub health: OutboundHealthObservation,
    pub current_latency_ms: OutboundObservation<u32>,
    pub latency_history: OutboundObservation<Vec<OutboundLatencySample>>,
    pub rtt: OutboundObservation<OutboundRttStatistics>,
    pub historical_egress: OutboundObservation<Vec<OutboundEgressSample>>,
    pub probe_availability: OutboundProbeAvailability,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutboundDetailsState {
    Unknown,
    Ready { details: Vec<OutboundDetail> },
    Empty,
    Unsupported,
    Failed { failure: Failure },
}

/// Immutable details observation fenced to one active core and detail revision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OutboundDetailsSnapshot {
    scope: CoreScope,
    detail_revision: SnapshotRevision,
    state: OutboundDetailsState,
}

impl OutboundDetailsSnapshot {
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

    pub fn scope(&self) -> &CoreScope {
        &self.scope
    }

    pub const fn detail_revision(&self) -> SnapshotRevision {
        self.detail_revision
    }

    pub fn state(&self) -> &OutboundDetailsState {
        &self.state
    }

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
