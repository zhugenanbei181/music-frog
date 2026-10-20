//! Engine-neutral outbound identity vocabulary.
//!
//! Every outbound-related identifier is scoped to the engine that owns it:
//! two engines may use the same wire name for different things, so an
//! identifier always carries its [`EngineId`] and is only ever meaningful
//! inside that namespace.

use crate::identity::EngineId;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};
use std::error::Error;
use std::fmt::{self, Display, Formatter};

const MAX_OUTBOUND_ID_BYTES: usize = 512;

/// Why an outbound-scoped value or snapshot was rejected.
///
/// Every variant is a contract violation of the outbound model: the snapshot
/// invariants (unique IDs, matching engine scope, matching members) are
/// checked before a snapshot is published, so a consumer never has to
/// re-validate them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OutboundContractError {
    /// The identifier was blank.
    EmptyId,
    /// The identifier was longer than 512 bytes.
    OversizedId,
    /// A `Ready` inventory carried no outbound at all.
    EmptyReadyInventory,
    /// A `Ready` details state carried no outbound at all.
    EmptyReadyDetails,
    /// An outbound, protocol or cipher belonged to another engine.
    EngineScopeMismatch,
    /// A collection referenced a member that the inventory does not contain.
    UnknownMember,
    /// The selected outbound was not a member of its own collection.
    SelectedOutboundNotMember,
    /// The inventory contained the same entry ID twice.
    DuplicateInventoryEntryId,
    /// The inventory contained the same collection ID twice.
    DuplicateSelectionCollectionId,
    /// One collection contained the same member ID twice.
    DuplicateSelectionMemberId,
    /// The details contained the same outbound ID twice.
    DuplicateOutboundDetailId,
    /// The UI preferences contained the same outbound ID twice.
    DuplicateOutboundPreferenceId,
    /// The probe batch contained the same result ID twice.
    DuplicateOutboundProbeResultId,
    /// The probe results did not match the requested probe scope.
    ProbeBatchResultMismatch,
    /// A probe target was not an absolute HTTP(S) URL.
    InvalidProbeUrl,
    /// A probe target carried embedded credentials.
    ProbeCredentialsNotAllowed,
    /// A probe timeout was zero.
    ZeroProbeTimeout,
    /// A probe batch concurrency was outside the supported range.
    InvalidProbeBatchConcurrency,
    /// A selected-outbound probe batch was empty.
    EmptySelectedProbeBatch,
    /// A selected-outbound probe batch repeated a target.
    DuplicateSelectedProbeTarget,
    /// Two observations that had to share one scope did not.
    ObservationScopeMismatch,
    /// The details did not match the members of the inventory.
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
    /// Builds an outbound ID inside one engine namespace.
    ///
    /// Fails with [`OutboundContractError::EmptyId`] for a blank value and
    /// with [`OutboundContractError::OversizedId`] for more than 512 bytes.
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

    /// Returns the engine namespace this ID belongs to.
    pub fn engine_id(&self) -> &EngineId {
        &self.engine_id
    }

    /// Borrows the opaque, adapter-owned ID value.
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
///
/// The revision is the ordering fence between two observations of the same
/// scope: a consumer must discard an observation whose revision is older than
/// the one it already rendered.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct SnapshotRevision(u64);

impl SnapshotRevision {
    /// Wraps a raw revision value.
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the raw revision value.
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
    /// Builds a protocol ID inside one engine namespace.
    ///
    /// Fails with [`OutboundContractError::EmptyId`] for a blank value and
    /// with [`OutboundContractError::OversizedId`] for more than 512 bytes.
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

    /// Returns the engine namespace this protocol ID belongs to.
    pub fn engine_id(&self) -> &EngineId {
        &self.engine_id
    }

    /// Borrows the opaque, adapter-owned protocol name.
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

/// Opaque engine-owned cipher identity, scoped to one engine namespace.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
pub struct OutboundCipherId {
    engine_id: EngineId,
    value: String,
}

impl OutboundCipherId {
    /// Builds a cipher ID inside one engine namespace.
    ///
    /// Fails with [`OutboundContractError::EmptyId`] for a blank value and
    /// with [`OutboundContractError::OversizedId`] for more than 512 bytes.
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

    /// Returns the engine namespace this cipher ID belongs to.
    pub fn engine_id(&self) -> &EngineId {
        &self.engine_id
    }

    /// Borrows the opaque, adapter-owned cipher name.
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
