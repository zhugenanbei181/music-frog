use super::details::OutboundSnapshot;
use super::details::{OutboundDetailsSnapshot, OutboundDetailsState};
use super::identity::{OutboundContractError, OutboundId, SnapshotRevision};
use crate::core_identity::CoreScope;
use crate::error::Failure;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashSet;

/// Whether a collection's current member can be changed through shared intent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SelectionAvailability {
    /// Shared application intent may select a member.
    Selectable,
    /// The active member is observed, but selection cannot be changed here.
    ReadOnly,
    /// The engine chooses the active member automatically.
    Automatic,
    /// The backend reports that selection control is unsupported.
    Unsupported,
    /// The backend has not reported selection capability.
    Unknown,
}

/// A selectable engine outbound, such as a Mihomo group or sing-box selector.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutboundSelectionSet {
    pub id: OutboundId,
    pub member_ids: Vec<OutboundId>,
    pub availability: SelectionAvailability,
    pub selection: OutboundSelectionState,
}

/// Observation state for the complete outbound inventory.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutboundInventoryState {
    Unknown,
    Ready {
        entries: Vec<OutboundSnapshot>,
        collections: Vec<OutboundSelectionSet>,
    },
    Empty,
    Unsupported,
    Failed {
        failure: Failure,
    },
}

/// Observation state for one selection set's current member.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutboundSelectionState {
    Unknown,
    Ready { selected: OutboundId },
    Empty,
    Unsupported,
    Failed { failure: Failure },
}

/// Observed result of probing one outbound.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutboundProbeState {
    Unknown,
    Probing,
    Measured { delay_ms: u32 },
    TimedOut,
    Unsupported,
    Failed { failure: Failure },
}

/// A probe result fenced to the core activation and observation revision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OutboundProbeSnapshot {
    scope: CoreScope,
    /// Inventory revision used for the membership compare-and-probe fence.
    observed_inventory_revision: SnapshotRevision,
    /// Details revision observed after the probe completed.
    detail_revision: SnapshotRevision,
    outbound_id: OutboundId,
    state: OutboundProbeState,
}

impl OutboundProbeSnapshot {
    pub fn new(
        scope: CoreScope,
        observed_inventory_revision: SnapshotRevision,
        detail_revision: SnapshotRevision,
        outbound_id: OutboundId,
        state: OutboundProbeState,
    ) -> Result<Self, OutboundContractError> {
        if outbound_id.engine_id() != &scope.identity.engine {
            return Err(OutboundContractError::EngineScopeMismatch);
        }
        Ok(Self {
            scope,
            observed_inventory_revision,
            detail_revision,
            outbound_id,
            state,
        })
    }

    pub fn scope(&self) -> &CoreScope {
        &self.scope
    }

    pub const fn observed_inventory_revision(&self) -> SnapshotRevision {
        self.observed_inventory_revision
    }

    pub const fn detail_revision(&self) -> SnapshotRevision {
        self.detail_revision
    }

    pub fn outbound_id(&self) -> &OutboundId {
        &self.outbound_id
    }

    pub fn state(&self) -> &OutboundProbeState {
        &self.state
    }
}

#[derive(Deserialize)]
struct OutboundProbeSnapshotFields {
    scope: CoreScope,
    observed_inventory_revision: SnapshotRevision,
    detail_revision: SnapshotRevision,
    outbound_id: OutboundId,
    state: OutboundProbeState,
}

impl<'de> Deserialize<'de> for OutboundProbeSnapshot {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let fields = OutboundProbeSnapshotFields::deserialize(deserializer)?;
        Self::new(
            fields.scope,
            fields.observed_inventory_revision,
            fields.detail_revision,
            fields.outbound_id,
            fields.state,
        )
        .map_err(D::Error::custom)
    }
}

/// Immutable inventory and selection observation fenced to one active core.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OutboundInventorySnapshot {
    scope: CoreScope,
    revision: SnapshotRevision,
    inventory: OutboundInventoryState,
}

impl OutboundInventorySnapshot {
    pub fn new(
        scope: CoreScope,
        revision: SnapshotRevision,
        inventory: OutboundInventoryState,
    ) -> Result<Self, OutboundContractError> {
        if let OutboundInventoryState::Ready {
            entries,
            collections,
        } = &inventory
        {
            if entries.is_empty() {
                return Err(OutboundContractError::EmptyReadyInventory);
            }
            let mut entry_ids = HashSet::new();
            for outbound in entries {
                if outbound.id.engine_id() != &scope.identity.engine {
                    return Err(OutboundContractError::EngineScopeMismatch);
                }
                if !entry_ids.insert(&outbound.id) {
                    return Err(OutboundContractError::DuplicateInventoryEntryId);
                }
            }
            let mut collection_ids = HashSet::new();
            for collection in collections {
                if collection.id.engine_id() != &scope.identity.engine {
                    return Err(OutboundContractError::EngineScopeMismatch);
                }
                if !collection_ids.insert(&collection.id) {
                    return Err(OutboundContractError::DuplicateSelectionCollectionId);
                }
                let mut member_ids = HashSet::new();
                for member in &collection.member_ids {
                    if member.engine_id() != &scope.identity.engine {
                        return Err(OutboundContractError::EngineScopeMismatch);
                    }
                    if !member_ids.insert(member) {
                        return Err(OutboundContractError::DuplicateSelectionMemberId);
                    }
                    if !entry_ids.contains(member) {
                        return Err(OutboundContractError::UnknownMember);
                    }
                }
                if let OutboundSelectionState::Ready { selected } = &collection.selection {
                    if selected.engine_id() != &scope.identity.engine {
                        return Err(OutboundContractError::EngineScopeMismatch);
                    }
                    if !collection.member_ids.contains(selected) {
                        return Err(OutboundContractError::SelectedOutboundNotMember);
                    }
                }
            }
        }
        Ok(Self {
            scope,
            revision,
            inventory,
        })
    }

    pub fn scope(&self) -> &CoreScope {
        &self.scope
    }

    pub const fn revision(&self) -> SnapshotRevision {
        self.revision
    }

    pub fn inventory(&self) -> &OutboundInventoryState {
        &self.inventory
    }
}

#[derive(Deserialize)]
struct OutboundInventorySnapshotFields {
    scope: CoreScope,
    revision: SnapshotRevision,
    inventory: OutboundInventoryState,
}

impl<'de> Deserialize<'de> for OutboundInventorySnapshot {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let fields = OutboundInventorySnapshotFields::deserialize(deserializer)?;
        Self::new(fields.scope, fields.revision, fields.inventory).map_err(D::Error::custom)
    }
}

/// One observation cycle containing inventory and details from the same
/// adapter read. The two revisions advance independently: metadata and latency
/// history cannot make an unchanged inventory stale for selection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OutboundObservationSnapshot {
    inventory: OutboundInventorySnapshot,
    details: OutboundDetailsSnapshot,
}

impl OutboundObservationSnapshot {
    pub fn new(
        inventory: OutboundInventorySnapshot,
        details: OutboundDetailsSnapshot,
    ) -> Result<Self, OutboundContractError> {
        if inventory.scope() != details.scope() {
            return Err(OutboundContractError::ObservationScopeMismatch);
        }
        match (inventory.inventory(), details.state()) {
            (
                OutboundInventoryState::Ready { entries, .. },
                OutboundDetailsState::Ready { details },
            ) => {
                if entries.len() != details.len()
                    || entries
                        .iter()
                        .any(|entry| !details.iter().any(|detail| detail.id == entry.id))
                {
                    return Err(OutboundContractError::InventoryDetailsMismatch);
                }
            }
            (OutboundInventoryState::Empty, OutboundDetailsState::Empty)
            | (_, OutboundDetailsState::Unknown | OutboundDetailsState::Unsupported)
            | (_, OutboundDetailsState::Failed { .. }) => {}
            _ => return Err(OutboundContractError::InventoryDetailsMismatch),
        }
        Ok(Self { inventory, details })
    }

    pub fn inventory(&self) -> &OutboundInventorySnapshot {
        &self.inventory
    }

    pub fn details(&self) -> &OutboundDetailsSnapshot {
        &self.details
    }
}

#[derive(Deserialize)]
struct OutboundObservationSnapshotFields {
    inventory: OutboundInventorySnapshot,
    details: OutboundDetailsSnapshot,
}

impl<'de> Deserialize<'de> for OutboundObservationSnapshot {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let fields = OutboundObservationSnapshotFields::deserialize(deserializer)?;
        Self::new(fields.inventory, fields.details).map_err(D::Error::custom)
    }
}
