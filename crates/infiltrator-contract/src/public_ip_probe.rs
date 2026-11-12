//! Typed owner vocabulary for public-egress probing.
//!
//! FlClash's pinned `RoutedProbe` publishes `empty`, `probing`, `fresh` and
//! `failed` and fences answers with a route stamp. This module keeps the
//! reusable semantics while expressing the lifecycle as one sealed state:
//! there is no private loading boolean and no combination of `Option` fields
//! that can encode two contradictory phases.

use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt::{self, Display, Formatter};

/// Typed lifecycle of one public-egress observation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicIpProbePhase {
    #[default]
    Idle,
    Probing,
    Succeeded,
    Failed,
}

impl PublicIpProbePhase {
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed)
    }
}

/// The route chosen by the owner when it stamps a refresh.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicIpProbeRouteStamp {
    #[default]
    Direct,
    ThroughRuntimeHttpProxy,
}

/// Correlation and route fence for one refresh.
///
/// `request_id` increases for every accepted refresh; `core_generation` and
/// `revision` are stamped at launch and checked again when the host answer
/// lands. A late answer from a different route cannot overwrite the readback.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicIpProbeStamp {
    pub request_id: u64,
    pub core_generation: u64,
    pub revision: u64,
    pub route: PublicIpProbeRouteStamp,
}

/// Typed request handed to the host probe port.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicIpProbeRequest {
    pub stamp: PublicIpProbeStamp,
}

impl PublicIpProbeRequest {
    pub const fn new(stamp: PublicIpProbeStamp) -> Self {
        Self { stamp }
    }
}

/// Facts the application owner may publish after a successful observation.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicIpObservation {
    pub ip: String,
    pub country: Option<String>,
    pub region: Option<String>,
    pub city: Option<String>,
    pub provider: String,
    pub checked_at_epoch_ms: u64,
}

impl PublicIpObservation {
    pub fn is_drawable(&self) -> bool {
        !self.ip.trim().is_empty() && self.checked_at_epoch_ms > 0
    }
}

/// One terminal answer from the host probe port, stamped identically to its
/// request. Failures are typed answers, not generic dispatch errors, so every
/// accepted refresh reaches a terminal readback.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PublicIpProbeAnswer {
    Succeeded {
        stamp: PublicIpProbeStamp,
        observation: PublicIpObservation,
    },
    Failed {
        stamp: PublicIpProbeStamp,
        reason: String,
    },
}

impl PublicIpProbeAnswer {
    pub const fn stamp(&self) -> PublicIpProbeStamp {
        match self {
            Self::Succeeded { stamp, .. } | Self::Failed { stamp, .. } => *stamp,
        }
    }
}

/// The owner's typed state. `#[serde(tag = "kind", rename_all = "snake_case")]`
/// makes invalid combinations such as `Ready` with a failure or `Probing`
/// with facts unrepresentable; the published `PublicIpProbeSnapshot` is only
/// a stable projection of this one state.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PublicIpProbeState {
    #[default]
    Idle,
    Probing {
        stamp: PublicIpProbeStamp,
    },
    Succeeded {
        stamp: PublicIpProbeStamp,
        observation: PublicIpObservation,
    },
    Failed {
        stamp: PublicIpProbeStamp,
        reason: String,
    },
    Stale {
        stamp: PublicIpProbeStamp,
    },
}

impl PublicIpProbeState {
    pub fn phase(&self) -> PublicIpProbePhase {
        match self {
            Self::Idle => PublicIpProbePhase::Idle,
            Self::Probing { .. } => PublicIpProbePhase::Probing,
            Self::Succeeded { .. } => PublicIpProbePhase::Succeeded,
            Self::Failed { .. } => PublicIpProbePhase::Failed,
            Self::Stale { .. } => PublicIpProbePhase::Idle,
        }
    }

    pub fn stamp(&self) -> Option<PublicIpProbeStamp> {
        match self {
            Self::Idle => None,
            Self::Probing { stamp }
            | Self::Succeeded { stamp, .. }
            | Self::Failed { stamp, .. }
            | Self::Stale { stamp } => Some(*stamp),
        }
    }

    pub fn begin(&self, stamp: PublicIpProbeStamp) -> Result<Self, PublicIpProbeError> {
        if matches!(self, Self::Probing { .. }) {
            return Err(PublicIpProbeError::AlreadyProbing);
        }
        Ok(Self::Probing { stamp })
    }

    /// Accept only the answer stamped for the currently in-flight request and
    /// route. A stale answer is fenced instead of becoming a second truth.
    pub fn accept(self, answer: PublicIpProbeAnswer) -> Result<Self, PublicIpProbeError> {
        let Some(active) = self.stamp() else {
            return Err(PublicIpProbeError::NoActiveRequest);
        };
        if !matches!(self, Self::Probing { .. }) {
            return Err(PublicIpProbeError::NotProbing);
        }
        if answer.stamp() != active {
            return Err(PublicIpProbeError::StaleAnswer);
        }
        Ok(match answer {
            PublicIpProbeAnswer::Succeeded {
                stamp, observation, ..
            } => Self::Succeeded { stamp, observation },
            PublicIpProbeAnswer::Failed { stamp, reason } => Self::Failed { stamp, reason },
        })
    }
}

/// Closed vocabulary for illegal owner transitions and stamped answer
/// admission.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PublicIpProbeError {
    NoActiveRequest,
    NotProbing,
    AlreadyProbing,
    StaleAnswer,
    Unconfigured,
    InvalidObservation,
}

impl Display for PublicIpProbeError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::NoActiveRequest => "no active public IP probe request",
            Self::NotProbing => "public IP probe answer arrived outside an active request",
            Self::AlreadyProbing => "public IP probe is already in flight",
            Self::StaleAnswer => "public IP probe answer belongs to a stale request or route",
            Self::Unconfigured => "public IP probe port is not composed",
            Self::InvalidObservation => "public IP probe returned an invalid observation",
        };
        formatter.write_str(message)
    }
}

impl Error for PublicIpProbeError {}

#[cfg(test)]
#[path = "public_ip_probe_test.rs"]
mod tests;
