//! Shared connection-telemetry vocabulary for both UI surfaces (DUAL-13).
//!
//! The live-connection feed has a lifecycle that both Iced and Bevy must be
//! able to describe to the user: idle, connecting, live, reconnecting, or
//! unavailable. Iced observes the WebSocket subscription directly; Bevy reads
//! the polled surface snapshot. They express the phase with the same enum so a
//! stream badge on either surface means the same thing.

use crate::capability::Availability;
use lurker_core_contract::identity::EngineId;
use lurker_core_contract::page::PageStatus;
use serde::de::Error as _;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::error::Error;
use std::fmt::{self, Display, Formatter};

const MAX_CONNECTION_ID_BYTES: usize = 512;

/// A connection identity scoped to the engine which issued it. The active
/// Core instance and generation are carried by `CoreScope` on every request.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
pub struct ConnectionId {
    engine_id: EngineId,
    value: String,
}

impl ConnectionId {
    pub fn new(
        engine_id: EngineId,
        value: impl Into<String>,
    ) -> Result<Self, ConnectionContractError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(ConnectionContractError::EmptyId);
        }
        if value.len() > MAX_CONNECTION_ID_BYTES {
            return Err(ConnectionContractError::OversizedId);
        }
        Ok(Self { engine_id, value })
    }

    pub fn engine_id(&self) -> &EngineId {
        &self.engine_id
    }

    pub fn value(&self) -> &str {
        &self.value
    }
}

#[derive(Deserialize)]
struct ConnectionIdFields {
    engine_id: EngineId,
    value: String,
}

impl<'de> Deserialize<'de> for ConnectionId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let fields = ConnectionIdFields::deserialize(deserializer)?;
        Self::new(fields.engine_id, fields.value).map_err(D::Error::custom)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConnectionContractError {
    EmptyId,
    OversizedId,
    EngineScopeMismatch,
    DuplicateConnectionId,
}

impl Display for ConnectionContractError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyId => formatter.write_str("connection ID must not be empty"),
            Self::OversizedId => formatter.write_str("connection ID exceeds 512 bytes"),
            Self::EngineScopeMismatch => {
                formatter.write_str("connection ID engine does not match Core scope")
            }
            Self::DuplicateConnectionId => {
                formatter.write_str("connection observation contains a duplicate ID")
            }
        }
    }
}

impl Error for ConnectionContractError {}

/// Validate the native connection IDs in a decoded controller observation.
/// The API payload stays a string at its wire boundary; Core operations use
/// `ConnectionId` after this validation.
pub fn validate_connection_ids(
    engine_id: &EngineId,
    values: impl IntoIterator<Item = String>,
) -> Result<Vec<ConnectionId>, ConnectionContractError> {
    let mut seen = HashSet::new();
    values
        .into_iter()
        .map(|value| {
            let id = ConnectionId::new(engine_id.clone(), value)?;
            if !seen.insert(id.value.clone()) {
                return Err(ConnectionContractError::DuplicateConnectionId);
            }
            Ok(id)
        })
        .collect()
}

/// Lifecycle phase of the live connections telemetry feed (DUAL-13-01).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionStreamPhase {
    /// No feed attached and no observation yet.
    #[default]
    Idle,
    /// The feed is attaching for the first time.
    Connecting,
    /// Fresh connection rows are arriving.
    Live,
    /// A previously live feed dropped and is retrying.
    Reconnecting,
    /// The host does not provide the connections feed.
    Unavailable,
}

/// Availability of a derived instantaneous connection rate.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionRateObservation {
    /// No valid two-snapshot counter window is available yet.
    #[default]
    Unobserved,
    /// Both byte counters came from a valid, monotonic counter window.
    Measured,
}

impl ConnectionStreamPhase {
    /// Canonical ordering used by the segmented badges on both surfaces.
    pub const ALL: [Self; 5] = [
        Self::Idle,
        Self::Connecting,
        Self::Live,
        Self::Reconnecting,
        Self::Unavailable,
    ];

    /// Stable wire/persistence identifier.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Connecting => "connecting",
            Self::Live => "live",
            Self::Reconnecting => "reconnecting",
            Self::Unavailable => "unavailable",
        }
    }

    /// Parse an identifier produced by [`Self::as_str`], defaulting to idle.
    pub fn from_identifier(value: &str) -> Self {
        match value {
            "connecting" => Self::Connecting,
            "live" => Self::Live,
            "reconnecting" => Self::Reconnecting,
            "unavailable" => Self::Unavailable,
            _ => Self::Idle,
        }
    }

    /// Whether the feed is actively delivering rows.
    pub const fn is_live(self) -> bool {
        matches!(self, Self::Live)
    }

    /// Derive the phase a polled surface can honestly claim from its page
    /// status. A ready or explicitly-empty page means rows were observed; a
    /// loading page is still connecting; an unavailable/failed page cannot be
    /// shown as live. Reconnecting is only observable by a streaming surface
    /// (Iced) and is never inferred from a single poll.
    pub fn from_page_status(status: &PageStatus) -> Self {
        match status {
            PageStatus::Loading => Self::Connecting,
            PageStatus::Ready | PageStatus::Empty => Self::Live,
            PageStatus::Unavailable { .. } | PageStatus::Failed { .. } => Self::Unavailable,
        }
    }
}

/// The controller payload carries counters and identity, but no staged timings.
pub fn timing_availability() -> Availability {
    Availability::Unsupported {
        reason: "The kernel does not report per-connection DNS/TCP/TLS/TTFB timings".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lurker_core_contract::error::Failure;

    #[test]
    fn rate_observation_serializes_as_a_typed_state() {
        assert_eq!(
            serde_json::to_string(&ConnectionRateObservation::Unobserved).unwrap(),
            "\"unobserved\""
        );
        assert_eq!(
            serde_json::to_string(&ConnectionRateObservation::Measured).unwrap(),
            "\"measured\""
        );
    }

    #[test]
    fn stream_phase_round_trips_its_identifier() {
        for phase in ConnectionStreamPhase::ALL {
            assert_eq!(
                ConnectionStreamPhase::from_identifier(phase.as_str()),
                phase
            );
        }
        assert_eq!(
            ConnectionStreamPhase::from_identifier("bogus"),
            ConnectionStreamPhase::Idle
        );
        assert!(ConnectionStreamPhase::Live.is_live());
        assert!(!ConnectionStreamPhase::Idle.is_live());
    }

    #[test]
    fn page_status_maps_to_an_honest_phase() {
        assert_eq!(
            ConnectionStreamPhase::from_page_status(&PageStatus::Loading),
            ConnectionStreamPhase::Connecting
        );
        assert_eq!(
            ConnectionStreamPhase::from_page_status(&PageStatus::Ready),
            ConnectionStreamPhase::Live
        );
        assert_eq!(
            ConnectionStreamPhase::from_page_status(&PageStatus::Empty),
            ConnectionStreamPhase::Live
        );
        assert_eq!(
            ConnectionStreamPhase::from_page_status(&PageStatus::Unavailable {
                failure: Failure::unsupported("no gateway")
            }),
            ConnectionStreamPhase::Unavailable
        );
    }
}
