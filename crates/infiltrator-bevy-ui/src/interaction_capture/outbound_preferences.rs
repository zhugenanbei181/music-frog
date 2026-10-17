//! Capture-only typed preference owner for native interaction replays.
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::outbound::{
    identity::SnapshotRevision,
    preferences::{
        OutboundFavoriteState, OutboundGroupExpansion, OutboundUiPreferenceIntent,
        OutboundUiPreferenceRequest, OutboundUiPreferencesSnapshot, OutboundUiPreferencesValues,
    },
};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub(super) struct OutboundPreferenceCaptureOwner(Arc<Mutex<OutboundUiPreferencesSnapshot>>);

impl OutboundPreferenceCaptureOwner {
    pub fn new(snapshot: OutboundUiPreferencesSnapshot) -> Self {
        Self(Arc::new(Mutex::new(snapshot)))
    }

    pub fn snapshot(&self) -> OutboundUiPreferencesSnapshot {
        self.0.lock().expect("capture preference owner").clone()
    }

    pub fn reset_search_query(&self) {
        let current = self.snapshot();
        let request = OutboundUiPreferenceRequest::new(
            current.revision(),
            OutboundUiPreferenceIntent::SetSearchQuery(String::new()),
        )
        .expect("empty capture search query is valid");
        self.apply(&request)
            .expect("capture fixture can reset its local search preference");
    }

    pub fn apply(
        &self,
        request: &OutboundUiPreferenceRequest,
    ) -> Result<OutboundUiPreferencesSnapshot, Failure> {
        let mut current = self.0.lock().expect("capture preference owner");
        if current.revision() != request.expected_revision() {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "capture preference request used a stale revision",
                true,
            ));
        }
        request
            .validate_scope(current.scope())
            .map_err(|error| Failure::new(ErrorCode::InvalidInput, error.to_string(), false))?;

        let mut search_query = current.search_query.clone();
        let mut favorite_outbound_ids = current.favorite_outbound_ids.clone();
        let mut group_order = current.group_order.clone();
        let mut collapsed_group_ids = current.collapsed_group_ids.clone();
        let mut sort_order = current.sort_order;
        let mut health_filter = current.health_filter;
        let mut layout = current.layout;
        match request.intent() {
            OutboundUiPreferenceIntent::SetSearchQuery(query) => search_query.clone_from(query),
            OutboundUiPreferenceIntent::SetFavorite { outbound_id, state } => match state {
                OutboundFavoriteState::Pinned => {
                    if !favorite_outbound_ids.contains(outbound_id) {
                        favorite_outbound_ids.push(outbound_id.clone());
                    }
                }
                OutboundFavoriteState::Unpinned => {
                    favorite_outbound_ids.retain(|id| id != outbound_id)
                }
            },
            OutboundUiPreferenceIntent::SetSortOrder(order) => sort_order = *order,
            OutboundUiPreferenceIntent::SetHealthFilter(filter) => health_filter = *filter,
            OutboundUiPreferenceIntent::SetLayout(value) => layout = *value,
            OutboundUiPreferenceIntent::SetGroupExpansion {
                group_id,
                expansion,
            } => match expansion {
                OutboundGroupExpansion::Expanded => collapsed_group_ids.retain(|id| id != group_id),
                OutboundGroupExpansion::Collapsed => {
                    if !collapsed_group_ids.contains(group_id) {
                        collapsed_group_ids.push(group_id.clone());
                    }
                }
            },
            OutboundUiPreferenceIntent::ReorderGroups { group_ids, .. } => {
                group_order.clone_from(group_ids)
            }
            OutboundUiPreferenceIntent::ResetGroupOrder { .. } => group_order.clear(),
        }
        let revision = current.revision().value().checked_add(1).ok_or_else(|| {
            Failure::new(
                ErrorCode::InvalidState,
                "capture preference revision is exhausted",
                false,
            )
        })?;
        let next = OutboundUiPreferencesSnapshot::new(
            current.scope().clone(),
            SnapshotRevision::new(revision),
            OutboundUiPreferencesValues {
                search_query,
                favorite_outbound_ids,
                group_order,
                collapsed_group_ids,
                sort_order,
                health_filter,
                layout,
            },
        )
        .map_err(|error| Failure::new(ErrorCode::InvalidInput, error.to_string(), false))?;
        *current = next.clone();
        Ok(next)
    }
}
