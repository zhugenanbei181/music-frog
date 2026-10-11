use super::identity::{OutboundContractError, OutboundId, SnapshotRevision};
use crate::core_identity::CoreScope;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashSet;

/// Engine-scoped presentation preferences using outbound identities rather
/// than controller names. Persistence and migration remain adapter concerns.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OutboundUiPreferencesSnapshot {
    scope: CoreScope,
    revision: SnapshotRevision,
    pub search_query: String,
    pub favorite_outbound_ids: Vec<OutboundId>,
    pub group_order: Vec<OutboundId>,
    pub collapsed_group_ids: Vec<OutboundId>,
    pub sort_order: OutboundSortOrder,
    pub health_filter: OutboundHealthFilter,
    pub layout: OutboundLayout,
}

/// Values for an engine-scoped outbound presentation preference snapshot.
/// Scope and revision remain snapshot metadata rather than presentation data.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OutboundUiPreferencesValues {
    pub search_query: String,
    pub favorite_outbound_ids: Vec<OutboundId>,
    pub group_order: Vec<OutboundId>,
    pub collapsed_group_ids: Vec<OutboundId>,
    pub sort_order: OutboundSortOrder,
    pub health_filter: OutboundHealthFilter,
    pub layout: OutboundLayout,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutboundSortOrder {
    #[default]
    LatencyAscending,
    LatencyDescending,
    NameAscending,
    NameDescending,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutboundHealthFilter {
    #[default]
    All,
    HealthyOnly,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutboundLayout {
    #[default]
    CompactList,
    Grid,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutboundGroupExpansion {
    Expanded,
    Collapsed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutboundFavoriteState {
    Pinned,
    Unpinned,
}

/// Exhaustive, typed write vocabulary for outbound presentation preferences.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", content = "value", rename_all = "snake_case")]
pub enum OutboundUiPreferenceIntent {
    SetSearchQuery(String),
    SetFavorite {
        outbound_id: OutboundId,
        state: OutboundFavoriteState,
    },
    SetSortOrder(OutboundSortOrder),
    SetHealthFilter(OutboundHealthFilter),
    SetLayout(OutboundLayout),
    SetGroupExpansion {
        group_id: OutboundId,
        expansion: OutboundGroupExpansion,
    },
    ReorderGroups {
        group_ids: Vec<OutboundId>,
        inventory_revision: SnapshotRevision,
    },
    ResetGroupOrder {
        inventory_revision: SnapshotRevision,
    },
}

impl OutboundUiPreferenceIntent {
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

    pub const fn expected_revision(&self) -> SnapshotRevision {
        self.expected_revision
    }

    pub fn intent(&self) -> &OutboundUiPreferenceIntent {
        &self.intent
    }

    pub fn validate_scope(&self, scope: &CoreScope) -> Result<(), OutboundContractError> {
        self.intent.validate_scope(scope)
    }

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

    pub fn scope(&self) -> &CoreScope {
        &self.scope
    }

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
