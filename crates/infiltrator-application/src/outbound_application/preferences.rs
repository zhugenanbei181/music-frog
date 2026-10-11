use super::*;

/// Scoped owner for typed outbound presentation preferences. The owner keeps
/// an in-memory readback until host persistence is migrated to this contract.
#[derive(Clone, Debug, Default)]
enum PreferenceOwnerState {
    #[default]
    Unbound,
    Bound(OutboundUiPreferencesSnapshot),
}

#[derive(Clone, Default)]
pub struct OutboundUiPreferencesApplication {
    state: Arc<Mutex<PreferenceOwnerState>>,
}

impl OutboundUiPreferencesApplication {
    pub fn new(scope: CoreScope) -> Self {
        Self {
            state: Arc::new(Mutex::new(PreferenceOwnerState::Bound(empty_preferences(
                scope,
            )))),
        }
    }

    pub fn with_initial(snapshot: OutboundUiPreferencesSnapshot) -> Self {
        Self {
            state: Arc::new(Mutex::new(PreferenceOwnerState::Bound(snapshot))),
        }
    }

    pub fn snapshot(&self, scope: &CoreScope) -> Result<OutboundUiPreferencesSnapshot, Failure> {
        let mut state = self.state.lock().map_err(|_| {
            Failure::new(
                ErrorCode::Internal,
                "outbound preference owner lock is poisoned",
                false,
            )
        })?;
        match &*state {
            PreferenceOwnerState::Bound(snapshot) if snapshot.scope() == scope => {
                Ok(snapshot.clone())
            }
            PreferenceOwnerState::Unbound | PreferenceOwnerState::Bound(_) => {
                let snapshot = empty_preferences(scope.clone());
                *state = PreferenceOwnerState::Bound(snapshot.clone());
                Ok(snapshot)
            }
        }
    }

    pub fn apply(
        &self,
        scope: &CoreScope,
        request: &OutboundUiPreferenceRequest,
        observation: &OutboundObservationSnapshot,
    ) -> Result<OutboundUiPreferencesSnapshot, Failure> {
        request.validate_scope(scope).map_err(|_| {
            invalid_observation("outbound preference request belongs to another engine")
        })?;
        verify_scope(scope, observation.inventory().scope())?;
        verify_scope(scope, observation.details().scope())?;
        validate_preference_targets(scope, request, observation)?;
        let mut state = self.state.lock().map_err(|_| {
            Failure::new(
                ErrorCode::Internal,
                "outbound preference owner lock is poisoned",
                false,
            )
        })?;
        let current = match &*state {
            PreferenceOwnerState::Bound(snapshot) if snapshot.scope() == scope => snapshot.clone(),
            PreferenceOwnerState::Unbound | PreferenceOwnerState::Bound(_) => {
                let snapshot = empty_preferences(scope.clone());
                *state = PreferenceOwnerState::Bound(snapshot.clone());
                snapshot
            }
        };
        if current.revision() != request.expected_revision() {
            return Err(invalid_observation(
                "outbound preference request uses a stale revision",
            ));
        }
        let revision = SnapshotRevision::new(
            current
                .revision()
                .value()
                .checked_add(1)
                .ok_or_else(|| invalid_observation("outbound preference revision exhausted"))?,
        );
        let mut search_query = current.search_query.clone();
        let mut favorite_outbound_ids = current.favorite_outbound_ids.clone();
        let mut group_order = current.group_order.clone();
        let mut collapsed_group_ids = current.collapsed_group_ids.clone();
        let mut sort_order = current.sort_order;
        let mut health_filter = current.health_filter;
        let mut layout = current.layout;
        match request.intent() {
            OutboundUiPreferenceIntent::SetSearchQuery(query) => search_query = query.clone(),
            OutboundUiPreferenceIntent::SetFavorite { outbound_id, state } => match state {
                OutboundFavoriteState::Pinned => {
                    if !favorite_outbound_ids.contains(outbound_id) {
                        favorite_outbound_ids.push(outbound_id.clone());
                    }
                }
                OutboundFavoriteState::Unpinned => {
                    favorite_outbound_ids.retain(|id| id != outbound_id);
                }
            },
            OutboundUiPreferenceIntent::SetSortOrder(order) => sort_order = *order,
            OutboundUiPreferenceIntent::SetHealthFilter(filter) => health_filter = *filter,
            OutboundUiPreferenceIntent::SetLayout(value) => layout = *value,
            OutboundUiPreferenceIntent::SetGroupExpansion {
                group_id,
                expansion,
            } => match expansion {
                OutboundGroupExpansion::Expanded => {
                    collapsed_group_ids.retain(|id| id != group_id);
                }
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
        let applied = OutboundUiPreferencesSnapshot::new(
            scope.clone(),
            revision,
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
        if !request.is_confirmed_by(&applied) {
            return Err(invalid_observation(
                "outbound preference readback did not confirm the requested value",
            ));
        }
        *state = PreferenceOwnerState::Bound(applied.clone());
        Ok(applied)
    }
}
