//! Engine-scoped outbound presentation preferences.
//!
//! The preferences are expressed with outbound identities rather than
//! controller names, so they stay valid across an engine restart and can be
//! persisted per engine. Writes are revision-fenced intents: the caller
//! states the revision it based its change on, and the write is only
//! confirmed when the resulting snapshot actually matches that intent.

use super::identity::{OutboundContractError, OutboundId, SnapshotRevision};
use crate::identity::CoreScope;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashSet;

/// Engine-scoped presentation preferences using outbound identities rather
/// than controller names. Persistence and migration remain adapter concerns.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OutboundUiPreferencesSnapshot {
    scope: CoreScope,
    revision: SnapshotRevision,
    /// The free-text filter the user typed into the outbound list.
    pub search_query: String,
    /// Outbounds the user pinned as favourites.
    pub favorite_outbound_ids: Vec<OutboundId>,
    /// The user-defined order of the groups.
    pub group_order: Vec<OutboundId>,
    /// Groups the user collapsed.
    pub collapsed_group_ids: Vec<OutboundId>,
    /// The order the outbound list is sorted by.
    pub sort_order: OutboundSortOrder,
    /// Which health states the list shows.
    pub health_filter: OutboundHealthFilter,
    /// The layout the outbound list uses.
    pub layout: OutboundLayout,
}

/// Values for an engine-scoped outbound presentation preference snapshot.
/// Scope and revision remain snapshot metadata rather than presentation data.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OutboundUiPreferencesValues {
    /// The free-text filter the user typed into the outbound list.
    pub search_query: String,
    /// Outbounds the user pinned as favourites.
    pub favorite_outbound_ids: Vec<OutboundId>,
    /// The user-defined order of the groups.
    pub group_order: Vec<OutboundId>,
    /// Groups the user collapsed.
    pub collapsed_group_ids: Vec<OutboundId>,
    /// The order the outbound list is sorted by.
    pub sort_order: OutboundSortOrder,
    /// Which health states the list shows.
    pub health_filter: OutboundHealthFilter,
    /// The layout the outbound list uses.
    pub layout: OutboundLayout,
}

/// The order the outbound list is sorted by.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutboundSortOrder {
    /// Sort by latency, fastest first.
    #[default]
    LatencyAscending,
    /// Sort by latency, slowest first.
    LatencyDescending,
    /// Sort by display name, ascending.
    NameAscending,
    /// Sort by display name, descending.
    NameDescending,
}

/// Which health states the outbound list shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutboundHealthFilter {
    /// Show every outbound.
    #[default]
    All,
    /// Show only outbounds the last probe reported as healthy.
    HealthyOnly,
}

/// The layout the outbound list uses.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutboundLayout {
    /// A dense single-column list.
    #[default]
    CompactList,
    /// A multi-column grid.
    Grid,
}

/// Whether a group is expanded or collapsed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutboundGroupExpansion {
    /// The group's members are visible.
    Expanded,
    /// The group is shown as a single collapsed row.
    Collapsed,
}

/// Whether an outbound is pinned as a favourite.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutboundFavoriteState {
    /// The outbound is pinned as a favourite.
    Pinned,
    /// The outbound is not pinned.
    Unpinned,
}

/// Exhaustive, typed write vocabulary for outbound presentation preferences.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", content = "value", rename_all = "snake_case")]
pub enum OutboundUiPreferenceIntent {
    /// Replace the free-text filter.
    SetSearchQuery(String),
    /// Pin or unpin one outbound.
    SetFavorite {
        /// The outbound whose favourite state changes.
        outbound_id: OutboundId,
        /// The new favourite state.
        state: OutboundFavoriteState,
    },
    /// Replace the sort order.
    SetSortOrder(OutboundSortOrder),
    /// Replace the health filter.
    SetHealthFilter(OutboundHealthFilter),
    /// Replace the layout.
    SetLayout(OutboundLayout),
    /// Expand or collapse one group.
    SetGroupExpansion {
        /// The group whose expansion state changes.
        group_id: OutboundId,
        /// The new expansion state.
        expansion: OutboundGroupExpansion,
    },
    /// Apply a new group order.
    ReorderGroups {
        /// The complete new group order.
        group_ids: Vec<OutboundId>,
        /// Inventory revision the order was computed against.
        inventory_revision: SnapshotRevision,
    },
    /// Drop the custom group order and fall back to the engine order.
    ResetGroupOrder {
        /// Inventory revision the reset was computed against.
        inventory_revision: SnapshotRevision,
    },
}

impl OutboundUiPreferenceIntent {
    /// Checks that every outbound identity in the intent belongs to `scope`.
    ///
    /// Fails with [`OutboundContractError::EngineScopeMismatch`] when an
    /// identity belongs to another engine, and with
    /// [`OutboundContractError::DuplicateOutboundPreferenceId`] when a group
    /// reorder repeats an identity.
    pub fn validate_scope(&self, scope: &CoreScope) -> Result<(), OutboundContractError> {
        let ids: Vec<&OutboundId> = match self {
            Self::SetFavorite { outbound_id, .. } => vec![outbound_id],
            Self::SetGroupExpansion { group_id, .. } => vec![group_id],
            Self::ReorderGroups { group_ids, .. } => group_ids.iter().collect(),
            Self::SetSearchQuery(_)
            | Self::SetSortOrder(_)
            | Self::SetHealthFilter(_)
            | Self::SetLayout(_)
            | Self::ResetGroupOrder { .. } => Vec::new(),
        };
        if ids
            .iter()
            .any(|id| id.engine_id() != &scope.identity.engine)
        {
            return Err(OutboundContractError::EngineScopeMismatch);
        }
        if let Self::ReorderGroups { group_ids: ids, .. } = self {
            let mut unique = HashSet::new();
            if ids.iter().any(|id| !unique.insert(id)) {
                return Err(OutboundContractError::DuplicateOutboundPreferenceId);
            }
        }
        Ok(())
    }
}

/// Revision-fenced request to update one outbound presentation preference.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OutboundUiPreferenceRequest {
    expected_revision: SnapshotRevision,
    intent: OutboundUiPreferenceIntent,
}

impl OutboundUiPreferenceRequest {
    /// Builds a revision-fenced preference request.
    ///
    /// Fails with [`OutboundContractError::DuplicateOutboundPreferenceId`]
    /// when a group reorder repeats an identity.
    pub fn new(
        expected_revision: SnapshotRevision,
        intent: OutboundUiPreferenceIntent,
    ) -> Result<Self, OutboundContractError> {
        if let OutboundUiPreferenceIntent::ReorderGroups { group_ids: ids, .. } = &intent {
            let mut unique = HashSet::new();
            if ids.iter().any(|id| !unique.insert(id)) {
                return Err(OutboundContractError::DuplicateOutboundPreferenceId);
            }
        }
        Ok(Self {
            expected_revision,
            intent,
        })
    }

    /// Returns the revision the request was based on.
    pub const fn expected_revision(&self) -> SnapshotRevision {
        self.expected_revision
    }

    /// Returns the intent the request carries.
    pub fn intent(&self) -> &OutboundUiPreferenceIntent {
        &self.intent
    }

    /// Checks that every outbound identity in the request belongs to `scope`.
    pub fn validate_scope(&self, scope: &CoreScope) -> Result<(), OutboundContractError> {
        self.intent.validate_scope(scope)
    }

    /// Whether `snapshot` is the result this request asked for.
    ///
    /// The snapshot must be newer than the expected revision, must belong to
    /// the same scope, and must actually carry the requested value. A stale
    /// or unrelated snapshot is never treated as a confirmation.
    pub fn is_confirmed_by(&self, snapshot: &OutboundUiPreferencesSnapshot) -> bool {
        if snapshot.revision() <= self.expected_revision
            || self.validate_scope(snapshot.scope()).is_err()
        {
            return false;
        }
        match &self.intent {
            OutboundUiPreferenceIntent::SetSearchQuery(query) => snapshot.search_query == *query,
            OutboundUiPreferenceIntent::SetFavorite { outbound_id, state } => match state {
                OutboundFavoriteState::Pinned => {
                    snapshot.favorite_outbound_ids.contains(outbound_id)
                }
                OutboundFavoriteState::Unpinned => {
                    !snapshot.favorite_outbound_ids.contains(outbound_id)
                }
            },
            OutboundUiPreferenceIntent::SetSortOrder(order) => snapshot.sort_order == *order,
            OutboundUiPreferenceIntent::SetHealthFilter(filter) => {
                snapshot.health_filter == *filter
            }
            OutboundUiPreferenceIntent::SetLayout(layout) => snapshot.layout == *layout,
            OutboundUiPreferenceIntent::SetGroupExpansion {
                group_id,
                expansion,
            } => match expansion {
                OutboundGroupExpansion::Expanded => {
                    !snapshot.collapsed_group_ids.contains(group_id)
                }
                OutboundGroupExpansion::Collapsed => {
                    snapshot.collapsed_group_ids.contains(group_id)
                }
            },
            OutboundUiPreferenceIntent::ReorderGroups { group_ids, .. } => {
                snapshot.group_order == *group_ids
            }
            OutboundUiPreferenceIntent::ResetGroupOrder { .. } => snapshot.group_order.is_empty(),
        }
    }
}

#[derive(Deserialize)]
struct OutboundUiPreferenceRequestFields {
    expected_revision: SnapshotRevision,
    intent: OutboundUiPreferenceIntent,
}

impl<'de> Deserialize<'de> for OutboundUiPreferenceRequest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let fields = OutboundUiPreferenceRequestFields::deserialize(deserializer)?;
        Self::new(fields.expected_revision, fields.intent).map_err(D::Error::custom)
    }
}

impl OutboundUiPreferencesSnapshot {
    /// Builds a preferences snapshot and validates it against its scope.
    ///
    /// Fails with [`OutboundContractError::EngineScopeMismatch`] when an
    /// identity belongs to another engine, and with
    /// [`OutboundContractError::DuplicateOutboundPreferenceId`] when a list
    /// repeats an identity.
    pub fn new(
        scope: CoreScope,
        revision: SnapshotRevision,
        values: OutboundUiPreferencesValues,
    ) -> Result<Self, OutboundContractError> {
        for ids in [
            &values.favorite_outbound_ids,
            &values.group_order,
            &values.collapsed_group_ids,
        ] {
            let mut unique = HashSet::new();
            for id in ids {
                if id.engine_id() != &scope.identity.engine {
                    return Err(OutboundContractError::EngineScopeMismatch);
                }
                if !unique.insert(id) {
                    return Err(OutboundContractError::DuplicateOutboundPreferenceId);
                }
            }
        }
        Ok(Self {
            scope,
            revision,
            search_query: values.search_query,
            favorite_outbound_ids: values.favorite_outbound_ids,
            group_order: values.group_order,
            collapsed_group_ids: values.collapsed_group_ids,
            sort_order: values.sort_order,
            health_filter: values.health_filter,
            layout: values.layout,
        })
    }

    /// Returns the core scope this snapshot is fenced to.
    pub fn scope(&self) -> &CoreScope {
        &self.scope
    }

    /// Returns the revision this snapshot was observed at.
    pub const fn revision(&self) -> SnapshotRevision {
        self.revision
    }
}

#[derive(Deserialize)]
struct OutboundUiPreferencesSnapshotFields {
    scope: CoreScope,
    revision: SnapshotRevision,
    search_query: String,
    favorite_outbound_ids: Vec<OutboundId>,
    group_order: Vec<OutboundId>,
    collapsed_group_ids: Vec<OutboundId>,
    sort_order: OutboundSortOrder,
    health_filter: OutboundHealthFilter,
    layout: OutboundLayout,
}

impl<'de> Deserialize<'de> for OutboundUiPreferencesSnapshot {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let fields = OutboundUiPreferencesSnapshotFields::deserialize(deserializer)?;
        Self::new(
            fields.scope,
            fields.revision,
            OutboundUiPreferencesValues {
                search_query: fields.search_query,
                favorite_outbound_ids: fields.favorite_outbound_ids,
                group_order: fields.group_order,
                collapsed_group_ids: fields.collapsed_group_ids,
                sort_order: fields.sort_order,
                health_filter: fields.health_filter,
                layout: fields.layout,
            },
        )
        .map_err(D::Error::custom)
    }
}
