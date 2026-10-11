use crate::core_identity::EngineId;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};
use std::error::Error;
use std::fmt::{self, Display, Formatter};

const MAX_OUTBOUND_ID_BYTES: usize = 512;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OutboundContractError {
    EmptyId,
    OversizedId,
    EmptyReadyInventory,
    EmptyReadyDetails,
    EngineScopeMismatch,
    UnknownMember,
    SelectedOutboundNotMember,
    DuplicateInventoryEntryId,
    DuplicateSelectionCollectionId,
    DuplicateSelectionMemberId,
    DuplicateOutboundDetailId,
    DuplicateOutboundPreferenceId,
    DuplicateOutboundProbeResultId,
    ProbeBatchResultMismatch,
    InvalidProbeUrl,
    ProbeCredentialsNotAllowed,
    ZeroProbeTimeout,
    InvalidProbeBatchConcurrency,
    EmptySelectedProbeBatch,
    DuplicateSelectedProbeTarget,
    ObservationScopeMismatch,
    InventoryDetailsMismatch,
}

impl Display for OutboundContractError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyId => formatter.write_str("outbound ID must not be empty"),
            Self::OversizedId => formatter.write_str("outbound ID exceeds 512 bytes"),
            Self::EmptyReadyInventory => {
                formatter.write_str("a ready outbound inventory must contain an outbound")
            }
            Self::EmptyReadyDetails => {
                formatter.write_str("ready outbound details must contain an outbound")
            }
            Self::EngineScopeMismatch => {
                formatter.write_str("outbound ID engine does not match snapshot scope")
            }
            Self::UnknownMember => {
                formatter.write_str("outbound collection references an unknown member")
            }
            Self::SelectedOutboundNotMember => {
                formatter.write_str("selected outbound is not a member of its collection")
            }
            Self::DuplicateInventoryEntryId => {
                formatter.write_str("outbound inventory contains a duplicate entry ID")
            }
            Self::DuplicateSelectionCollectionId => {
                formatter.write_str("outbound inventory contains a duplicate collection ID")
            }
            Self::DuplicateSelectionMemberId => {
                formatter.write_str("outbound collection contains a duplicate member ID")
            }
            Self::DuplicateOutboundDetailId => {
                formatter.write_str("outbound details contain a duplicate outbound ID")
            }
            Self::DuplicateOutboundPreferenceId => {
                formatter.write_str("outbound UI preferences contain a duplicate outbound ID")
            }
            Self::DuplicateOutboundProbeResultId => {
                formatter.write_str("outbound probe batch contains a duplicate result ID")
            }
            Self::ProbeBatchResultMismatch => {
                formatter.write_str("outbound probe batch results do not match the requested scope")
            }
            Self::InvalidProbeUrl => {
                formatter.write_str("probe target must be an absolute HTTP(S) URL")
            }
            Self::ProbeCredentialsNotAllowed => {
                formatter.write_str("probe target must not contain credentials")
            }
            Self::ZeroProbeTimeout => {
                formatter.write_str("probe timeout must be greater than zero")
            }
            Self::InvalidProbeBatchConcurrency => {
                formatter.write_str("outbound probe batch concurrency must be between 1 and 30")
            }
            Self::EmptySelectedProbeBatch => {
                formatter.write_str("a selected outbound probe batch must not be empty")
            }
            Self::DuplicateSelectedProbeTarget => {
                formatter.write_str("a selected outbound probe batch contains a duplicate target")
            }
            Self::ObservationScopeMismatch => {
                formatter.write_str("outbound inventory and details use different core scopes")
            }
            Self::InventoryDetailsMismatch => {
                formatter.write_str("outbound details do not match the inventory members")
            }
        }
    }
}

impl Error for OutboundContractError {}

/// Opaque outbound identity, valid only within the named engine namespace.
///
/// The value is preserved exactly; adapters own any interpretation of it.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
pub struct OutboundId {
    engine_id: EngineId,
    value: String,
}

impl OutboundId {
    pub fn new(
        engine_id: EngineId,
        value: impl Into<String>,
    ) -> Result<Self, OutboundContractError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(OutboundContractError::EmptyId);
        }
        if value.len() > MAX_OUTBOUND_ID_BYTES {
            return Err(OutboundContractError::OversizedId);
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
struct OutboundIdFields {
    engine_id: EngineId,
    value: String,
}

impl<'de> Deserialize<'de> for OutboundId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let fields = OutboundIdFields::deserialize(deserializer)?;
        Self::new(fields.engine_id, fields.value).map_err(D::Error::custom)
    }
}

/// Revision of one engine-scoped observation.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct SnapshotRevision(u64);

impl SnapshotRevision {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

/// Opaque engine-owned protocol identity. The UI-facing label remains a
/// separate inventory fact so equal wire names do not imply equal semantics.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
pub struct OutboundProtocolId {
    engine_id: EngineId,
    value: String,
}

impl OutboundProtocolId {
    pub fn new(
        engine_id: EngineId,
        value: impl Into<String>,
    ) -> Result<Self, OutboundContractError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(OutboundContractError::EmptyId);
        }
        if value.len() > MAX_OUTBOUND_ID_BYTES {
            return Err(OutboundContractError::OversizedId);
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
struct OutboundProtocolIdFields {
    engine_id: EngineId,
    value: String,
}

impl<'de> Deserialize<'de> for OutboundProtocolId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let fields = OutboundProtocolIdFields::deserialize(deserializer)?;
        Self::new(fields.engine_id, fields.value).map_err(D::Error::custom)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
pub struct OutboundCipherId {
    engine_id: EngineId,
    value: String,
}

impl OutboundCipherId {
    pub fn new(
        engine_id: EngineId,
        value: impl Into<String>,
    ) -> Result<Self, OutboundContractError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(OutboundContractError::EmptyId);
        }
        if value.len() > MAX_OUTBOUND_ID_BYTES {
            return Err(OutboundContractError::OversizedId);
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
struct OutboundCipherIdFields {
    engine_id: EngineId,
    value: String,
}

impl<'de> Deserialize<'de> for OutboundCipherId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let fields = OutboundCipherIdFields::deserialize(deserializer)?;
        Self::new(fields.engine_id, fields.value).map_err(D::Error::custom)
    }
}
