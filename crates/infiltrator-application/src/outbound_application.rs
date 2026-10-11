//! Shared outbound inventory, selection, and probe use-cases.

use futures_util::stream::{self, StreamExt};
use infiltrator_contract::core_identity::CoreScope;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::outbound::inventory::OutboundProbeState;
use infiltrator_contract::outbound::{
    details::{
        OutboundDetailsSnapshot, OutboundDetailsState, OutboundObservation,
        OutboundProbeAvailability,
    },
    identity::{OutboundId, SnapshotRevision},
    inventory::{
        OutboundInventorySnapshot, OutboundInventoryState, OutboundObservationSnapshot,
        OutboundProbeSnapshot, OutboundSelectionSet, OutboundSelectionState, SelectionAvailability,
    },
    preferences::{
        OutboundFavoriteState, OutboundGroupExpansion, OutboundHealthFilter, OutboundLayout,
        OutboundSortOrder, OutboundUiPreferenceIntent, OutboundUiPreferenceRequest,
        OutboundUiPreferencesSnapshot, OutboundUiPreferencesValues,
    },
    probe::{
        OutboundProbeBatchItem, OutboundProbeBatchOutcome, OutboundProbeBatchScope,
        OutboundProbeBatchSnapshot, OutboundSelectionRequest, ProbeOutboundBatchRequest,
        ProbeOutboundRequest,
    },
};
use infiltrator_ports::outbound::{OutboundPort, OutboundProbeRequest, ProbeTarget};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

#[path = "outbound_application/preferences.rs"]
pub mod preferences;

#[derive(Clone)]
pub struct OutboundApplication {
    port: Arc<dyn OutboundPort>,
}

impl OutboundApplication {
    pub fn new(port: Arc<dyn OutboundPort>) -> Self {
        Self { port }
    }

    /// Read the adapter-authored inventory for the requested core activation.
    pub async fn inventory(&self, scope: CoreScope) -> Result<OutboundInventorySnapshot, Failure> {
        let snapshot = self
            .port
            .inventory(scope.clone())
            .await
            .map_err(Failure::from)?;
        verify_scope(&scope, snapshot.scope())?;
        Ok(snapshot)
    }

    /// Read one coherent adapter observation cycle and verify its scope fence.
    pub async fn observation(
        &self,
        scope: CoreScope,
    ) -> Result<OutboundObservationSnapshot, Failure> {
        let snapshot = self
            .port
            .observation(scope.clone())
            .await
            .map_err(Failure::from)?;
        verify_scope(&scope, snapshot.inventory().scope())?;
        verify_scope(&scope, snapshot.details().scope())?;
        Ok(snapshot)
    }

    /// Read typed details aligned with an exact inventory observation. The
    /// adapter must return details for every ID in the same scope. Detail and
    /// inventory revisions advance independently.
    pub async fn details(
        &self,
        inventory: &OutboundInventorySnapshot,
    ) -> Result<OutboundDetailsSnapshot, Failure> {
        let snapshot = self
            .port
            .details(inventory.scope().clone(), inventory.revision())
            .await
            .map_err(Failure::from)?;
        verify_scope(inventory.scope(), snapshot.scope())?;
        match (inventory.inventory(), snapshot.state()) {
            (OutboundInventoryState::Empty, OutboundDetailsState::Empty) => Ok(snapshot),
            (
                OutboundInventoryState::Ready { entries, .. },
                OutboundDetailsState::Ready { details },
            ) => {
                if entries.len() != details.len()
                    || entries
                        .iter()
                        .any(|entry| snapshot.detail(&entry.id).is_none())
                {
                    return Err(invalid_observation(
                        "outbound details do not cover the observed inventory",
                    ));
                }
                Ok(snapshot)
            }
            (_, OutboundDetailsState::Failed { failure }) => Err(failure.clone()),
            (_, OutboundDetailsState::Unsupported) => Err(Failure::unsupported(
                "outbound details are unsupported by the active backend",
            )),
            (_, OutboundDetailsState::Unknown) => Err(Failure::new(
                ErrorCode::NotReady,
                "outbound details have not been observed",
                true,
            )),
            _ => Err(invalid_observation(
                "outbound inventory and details have incompatible states",
            )),
        }
    }

    /// Select from a surface request carrying only opaque IDs and the revision
    /// it observed. The application refreshes inventory before dispatch, so a
    /// stale request or an unavailable collection fails before the write port.
    pub async fn select_request(
        &self,
        scope: &CoreScope,
        request: &OutboundSelectionRequest,
    ) -> Result<OutboundInventorySnapshot, Failure> {
        request
            .validate_scope(scope)
            .map_err(|_| invalid_observation("outbound selection belongs to another engine"))?;
        let observed = self.inventory(scope.clone()).await?;
        if observed.revision() != request.expected_revision() {
            return Err(invalid_observation(
                "outbound selection request uses a stale snapshot revision",
            ));
        }
        let collection =
            selectable_collection(&observed, request.collection_id(), request.outbound_id())?;
        if matches!(
            &collection.selection,
            OutboundSelectionState::Ready { selected }
                if selected == request.outbound_id()
        ) {
            return Ok(observed);
        }

        let snapshot = self
            .port
            .select(
                scope.clone(),
                request.expected_revision(),
                request.collection_id().clone(),
                request.outbound_id().clone(),
            )
            .await
            .map_err(Failure::from)?;
        verify_scope(scope, snapshot.scope())?;
        verify_newer_revision(request.expected_revision(), snapshot.revision())?;
        verify_selection(&snapshot, request.collection_id(), request.outbound_id())?;
        Ok(snapshot)
    }

    /// Probe an outbound through a cross-surface request fenced to the
    /// inventory revision that the requesting surface observed.
    pub async fn probe_request(
        &self,
        scope: &CoreScope,
        request: &ProbeOutboundRequest,
    ) -> Result<OutboundProbeSnapshot, Failure> {
        request
            .validate_scope(scope)
            .map_err(|_| invalid_observation("outbound probe belongs to another engine"))?;
        let observation = self.observation(scope.clone()).await?;
        let observed = observation.inventory();
        if observed.revision() != request.expected_inventory_revision() {
            return Err(invalid_observation(
                "outbound probe request uses a stale inventory revision",
            ));
        }
        require_inventory_member(observed, request.outbound_id())?;
        let details = observation.details();
        let detail = details.detail(request.outbound_id()).ok_or_else(|| {
            Failure::new(
                ErrorCode::InvalidInput,
                "outbound details were not observed for the requested outbound",
                false,
            )
        })?;
        require_probe_supported(detail.probe_availability)?;
        let target = ProbeTarget::http_url(request.test_url().to_owned())
            .map_err(|error| Failure::new(ErrorCode::InvalidInput, error.to_string(), false))?;
        let adapter_request = OutboundProbeRequest::new(target, request.timeout_ms())
            .map_err(|error| Failure::new(ErrorCode::InvalidInput, error.to_string(), false))?;
        let probe = self
            .port
            .probe(
                scope.clone(),
                request.expected_inventory_revision(),
                request.outbound_id().clone(),
                adapter_request,
            )
            .await
            .map_err(Failure::from)?;
        verify_scope(scope, probe.scope())?;
        if !request.is_confirmed_by(&probe) || probe.detail_revision() <= details.detail_revision()
        {
            return Err(invalid_observation(
                "outbound probe result does not confirm the requested outbound, inventory revision, and detail readback",
            ));
        }
        Ok(probe)
    }

    /// Probe one, a collection's members, or the complete observed inventory
    /// from one coherent inventory/details read. Every target is validated
    /// before any probe operation begins, and supported members fan out under
    /// the request's bounded concurrency limit.
    pub async fn probe_batch_request(
        &self,
        scope: &CoreScope,
        request: &ProbeOutboundBatchRequest,
    ) -> Result<OutboundProbeBatchSnapshot, Failure> {
        request
            .validate_scope(scope)
            .map_err(|_| invalid_observation("outbound probe batch belongs to another engine"))?;
        let target = ProbeTarget::http_url(request.test_url().to_owned())
            .map_err(|error| Failure::new(ErrorCode::InvalidInput, error.to_string(), false))?;
        let adapter_request = OutboundProbeRequest::new(target, request.timeout_ms())
            .map_err(|error| Failure::new(ErrorCode::InvalidInput, error.to_string(), false))?;

        let observation = self.observation(scope.clone()).await?;
        let inventory = observation.inventory();
        if inventory.revision() != request.expected_inventory_revision() {
            return Err(invalid_observation(
                "outbound probe batch request uses a stale inventory revision",
            ));
        }

        let mut target_ids = probe_batch_target_ids(inventory, request.target_scope())?;
        let mut seen_ids = HashSet::new();
        for outbound_id in &target_ids {
            if outbound_id.engine_id() != &scope.identity.engine || !seen_ids.insert(outbound_id) {
                return Err(invalid_observation(
                    "outbound probe batch contains a mismatched or duplicate member",
                ));
            }
        }

        let details = observation.details();
        let detail_list = match details.state() {
            OutboundDetailsState::Ready { details } => details,
            OutboundDetailsState::Empty if target_ids.is_empty() => {
                return OutboundProbeBatchSnapshot::new(
                    scope.clone(),
                    request.clone(),
                    details.detail_revision(),
                    Vec::new(),
                )
                .map_err(|error| invalid_observation(error.to_string()));
            }
            OutboundDetailsState::Empty => {
                return Err(invalid_observation(
                    "outbound probe batch details are empty for observed members",
                ));
            }
            OutboundDetailsState::Unsupported => {
                return Err(Failure::unsupported(
                    "outbound probe batch requires backend-authored probe availability",
                ));
            }
            OutboundDetailsState::Unknown => {
                return Err(Failure::new(
                    ErrorCode::NotReady,
                    "outbound probe batch details have not been observed",
                    true,
                ));
            }
            OutboundDetailsState::Failed { failure } => return Err(failure.clone()),
        };

        for outbound_id in &target_ids {
            if !detail_list.iter().any(|detail| &detail.id == outbound_id) {
                return Err(invalid_observation(
                    "outbound details omitted a batch probe member",
                ));
            }
        }
        if matches!(
            request.target_scope(),
            OutboundProbeBatchScope::Collection(_) | OutboundProbeBatchScope::All
        ) {
            let collection_ids: HashSet<_> = match inventory.inventory() {
                OutboundInventoryState::Ready { collections, .. } => collections
                    .iter()
                    .map(|collection| &collection.id)
                    .collect(),
                _ => HashSet::new(),
            };
            target_ids.retain(|outbound_id| {
                !collection_ids.contains(outbound_id)
                    && detail_list
                        .iter()
                        .find(|detail| &detail.id == outbound_id)
                        .is_some_and(|detail| {
                            !matches!(detail.collection_kind, OutboundObservation::Observed(_))
                        })
            });
        }

        let mut items: Vec<Option<OutboundProbeBatchItem>> =
            (0..target_ids.len()).map(|_| None).collect();
        let mut supported = Vec::new();
        for (index, outbound_id) in target_ids.into_iter().enumerate() {
            let Some(detail) = detail_list.iter().find(|detail| detail.id == outbound_id) else {
                return Err(invalid_observation(
                    "outbound details omitted a batch probe member",
                ));
            };
            match detail.probe_availability {
                OutboundProbeAvailability::Supported => supported.push((index, outbound_id)),
                OutboundProbeAvailability::Unsupported => {
                    items[index] = Some(OutboundProbeBatchItem {
                        outbound_id,
                        outcome: OutboundProbeBatchOutcome::Unsupported,
                    });
                }
                OutboundProbeAvailability::Unknown => {
                    items[index] = Some(OutboundProbeBatchItem {
                        outbound_id,
                        outcome: OutboundProbeBatchOutcome::Failed {
                            failure: Failure::new(
                                ErrorCode::NotReady,
                                "outbound probe availability has not been observed",
                                true,
                            ),
                        },
                    });
                }
            }
        }

        let expected_revision = request.expected_inventory_revision();
        let observed_detail_revision = details.detail_revision();
        let probe_scope = scope.clone();
        let probe_request = adapter_request;
        let port = self.port.clone();
        let mut completed: Vec<Result<(usize, OutboundProbeBatchItem, SnapshotRevision), Failure>> =
            stream::iter(supported.into_iter().map(|(index, outbound_id)| {
                let scope = probe_scope.clone();
                let request = probe_request.clone();
                let port = port.clone();
                async move {
                    let probe = match port
                        .probe(scope.clone(), expected_revision, outbound_id.clone(), request)
                        .await
                    {
                        Ok(probe) => probe,
                        Err(error) => {
                            return Ok((
                                index,
                                OutboundProbeBatchItem {
                                    outbound_id,
                                    outcome: OutboundProbeBatchOutcome::Failed {
                                        failure: Failure::from(error),
                                    },
                                },
                                observed_detail_revision,
                            ));
                        }
                    };
                    verify_scope(&scope, probe.scope())?;
                    if probe.outbound_id() != &outbound_id
                        || probe.observed_inventory_revision() != expected_revision
                        || probe.detail_revision() <= observed_detail_revision
                    {
                        return Err(invalid_observation(
                            "outbound probe batch result does not confirm its member and observed revisions",
                        ));
                    }
                    let outcome = match probe.state() {
                        OutboundProbeState::Measured {
                            delay_ms,
                        } => OutboundProbeBatchOutcome::Measured {
                            delay_ms: *delay_ms,
                        },
                        OutboundProbeState::TimedOut => {
                            OutboundProbeBatchOutcome::TimedOut
                        }
                        OutboundProbeState::Unsupported => {
                            OutboundProbeBatchOutcome::Unsupported
                        }
                        OutboundProbeState::Failed { failure } => {
                            OutboundProbeBatchOutcome::Failed {
                                failure: failure.clone(),
                            }
                        }
                        OutboundProbeState::Unknown
                        | OutboundProbeState::Probing => {
                            OutboundProbeBatchOutcome::Failed {
                                failure: invalid_observation(
                                    "outbound probe adapter returned a nonterminal batch result",
                                ),
                            }
                        }
                    };
                    Ok((
                        index,
                        OutboundProbeBatchItem {
                            outbound_id,
                            outcome,
                        },
                        probe.detail_revision(),
                    ))
                }
            }))
            .buffer_unordered(request.concurrency().limit())
            .collect()
            .await;
        completed.sort_by_key(|result| result.as_ref().map_or(usize::MAX, |(index, _, _)| *index));

        let mut result_detail_revision = observed_detail_revision;
        for result in completed {
            let (index, item, detail_revision) = result?;
            result_detail_revision = result_detail_revision.max(detail_revision);
            items[index] = Some(item);
        }

        let items = items
            .into_iter()
            .map(|item| {
                item.ok_or_else(|| invalid_observation("outbound probe batch omitted a member"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        OutboundProbeBatchSnapshot::new(
            scope.clone(),
            request.clone(),
            result_detail_revision,
            items,
        )
        .map_err(|error| invalid_observation(error.to_string()))
    }
}

fn empty_preferences(scope: CoreScope) -> OutboundUiPreferencesSnapshot {
    OutboundUiPreferencesSnapshot::new(
        scope,
        SnapshotRevision::new(1),
        OutboundUiPreferencesValues {
            search_query: String::new(),
            favorite_outbound_ids: Vec::new(),
            group_order: Vec::new(),
            collapsed_group_ids: Vec::new(),
            sort_order: OutboundSortOrder::default(),
            health_filter: OutboundHealthFilter::default(),
            layout: OutboundLayout::default(),
        },
    )
    .expect("empty outbound preferences are valid")
}

fn selectable_collection<'a>(
    snapshot: &'a OutboundInventorySnapshot,
    collection_id: &OutboundId,
    outbound_id: &OutboundId,
) -> Result<&'a OutboundSelectionSet, Failure> {
    let OutboundInventoryState::Ready { collections, .. } = snapshot.inventory() else {
        return Err(invalid_observation(
            "outbound inventory is not ready for selection",
        ));
    };
    let collection = collections
        .iter()
        .find(|collection| &collection.id == collection_id)
        .ok_or_else(|| {
            Failure::new(
                ErrorCode::InvalidInput,
                "outbound selection collection was not observed",
                false,
            )
        })?;
    if !collection.member_ids.contains(outbound_id) {
        return Err(Failure::new(
            ErrorCode::InvalidInput,
            "outbound is not a member of the observed selection collection",
            false,
        ));
    }
    match collection.availability {
        SelectionAvailability::Selectable => Ok(collection),
        SelectionAvailability::Unsupported => Err(Failure::unsupported(
            "outbound selection is unsupported by the active backend",
        )),
        SelectionAvailability::Unknown => Err(Failure::new(
            ErrorCode::NotReady,
            "outbound selection availability has not been observed",
            true,
        )),
        SelectionAvailability::ReadOnly | SelectionAvailability::Automatic => Err(Failure::new(
            ErrorCode::InvalidState,
            "observed outbound collection does not allow manual selection",
            false,
        )),
    }
}

fn require_inventory_member(
    snapshot: &OutboundInventorySnapshot,
    outbound_id: &OutboundId,
) -> Result<(), Failure> {
    let OutboundInventoryState::Ready { entries, .. } = snapshot.inventory() else {
        return Err(invalid_observation(
            "outbound inventory is not ready for probing",
        ));
    };
    if entries.iter().any(|entry| &entry.id == outbound_id) {
        return Ok(());
    }
    Err(Failure::new(
        ErrorCode::InvalidInput,
        "outbound was not observed in the current inventory",
        false,
    ))
}

fn probe_batch_target_ids(
    snapshot: &OutboundInventorySnapshot,
    target_scope: &OutboundProbeBatchScope,
) -> Result<Vec<OutboundId>, Failure> {
    match snapshot.inventory() {
        OutboundInventoryState::Ready {
            entries,
            collections,
        } => match target_scope {
            OutboundProbeBatchScope::One(outbound_id) => {
                if entries.iter().any(|entry| &entry.id == outbound_id) {
                    Ok(vec![outbound_id.clone()])
                } else {
                    Err(Failure::new(
                        ErrorCode::InvalidInput,
                        "outbound probe target is absent from the observed inventory",
                        false,
                    ))
                }
            }
            OutboundProbeBatchScope::Collection(collection_id) => {
                let collection = collections
                    .iter()
                    .find(|collection| &collection.id == collection_id)
                    .ok_or_else(|| {
                        Failure::new(
                            ErrorCode::InvalidInput,
                            "outbound probe collection is absent from the observed inventory",
                            false,
                        )
                    })?;
                if collection
                    .member_ids
                    .iter()
                    .any(|member| !entries.iter().any(|entry| &entry.id == member))
                {
                    return Err(invalid_observation(
                        "outbound probe collection contains an unobserved member",
                    ));
                }
                Ok(collection.member_ids.clone())
            }
            OutboundProbeBatchScope::Selected(outbound_ids) => {
                let collection_ids: HashSet<_> = collections
                    .iter()
                    .map(|collection| &collection.id)
                    .collect();
                let mut targets = Vec::with_capacity(outbound_ids.len());
                for outbound_id in outbound_ids {
                    if !entries.iter().any(|entry| &entry.id == outbound_id) {
                        return Err(Failure::new(
                            ErrorCode::InvalidInput,
                            "selected outbound probe target is absent from the observed inventory",
                            false,
                        ));
                    }
                    if collection_ids.contains(outbound_id) {
                        return Err(Failure::new(
                            ErrorCode::InvalidInput,
                            "selected outbound probe target is a collection, not a concrete outbound",
                            false,
                        ));
                    }
                    targets.push(outbound_id.clone());
                }
                Ok(targets)
            }
            OutboundProbeBatchScope::All => {
                Ok(entries.iter().map(|entry| entry.id.clone()).collect())
            }
        },
        OutboundInventoryState::Empty if matches!(target_scope, OutboundProbeBatchScope::All) => {
            Ok(Vec::new())
        }
        OutboundInventoryState::Empty => Err(Failure::new(
            ErrorCode::InvalidInput,
            "outbound probe target is absent from the empty inventory",
            false,
        )),
        OutboundInventoryState::Unsupported => Err(Failure::unsupported(
            "outbound probe batch requires a supported inventory",
        )),
        OutboundInventoryState::Unknown => Err(Failure::new(
            ErrorCode::NotReady,
            "outbound inventory has not been observed for batch probing",
            true,
        )),
        OutboundInventoryState::Failed { failure } => Err(failure.clone()),
    }
}

fn validate_preference_targets(
    _scope: &CoreScope,
    request: &OutboundUiPreferenceRequest,
    observation: &OutboundObservationSnapshot,
) -> Result<(), Failure> {
    let needs_member = matches!(
        request.intent(),
        OutboundUiPreferenceIntent::SetFavorite { .. }
    );
    let inventory_revision = match request.intent() {
        OutboundUiPreferenceIntent::ReorderGroups {
            inventory_revision, ..
        }
        | OutboundUiPreferenceIntent::ResetGroupOrder { inventory_revision } => {
            Some(*inventory_revision)
        }
        _ => None,
    };
    if inventory_revision.is_some_and(|expected| expected != observation.inventory().revision()) {
        return Err(invalid_observation(
            "group order request uses a stale outbound inventory revision",
        ));
    }
    let needs_collection = matches!(
        request.intent(),
        OutboundUiPreferenceIntent::SetGroupExpansion { .. }
            | OutboundUiPreferenceIntent::ReorderGroups { .. }
    );
    if !needs_member && !needs_collection {
        return Ok(());
    }
    let (entries, collections) = match observation.inventory().inventory() {
        OutboundInventoryState::Ready {
            entries,
            collections,
        } => (entries, collections),
        OutboundInventoryState::Failed { failure } => return Err(failure.clone()),
        OutboundInventoryState::Unsupported => {
            return Err(Failure::unsupported(
                "outbound preferences need an observed inventory",
            ));
        }
        OutboundInventoryState::Unknown => {
            return Err(Failure::new(
                ErrorCode::NotReady,
                "outbound inventory is unknown; preference targets cannot be validated",
                true,
            ));
        }
        OutboundInventoryState::Empty => {
            return Err(Failure::new(
                ErrorCode::InvalidInput,
                "outbound preference target is absent from the empty inventory",
                false,
            ));
        }
    };
    match request.intent() {
        OutboundUiPreferenceIntent::SetFavorite { outbound_id, .. } => {
            if entries.iter().any(|entry| &entry.id == outbound_id) {
                Ok(())
            } else {
                Err(Failure::new(
                    ErrorCode::InvalidInput,
                    "favorite outbound is not present in the observed inventory",
                    false,
                ))
            }
        }
        OutboundUiPreferenceIntent::SetGroupExpansion { group_id, .. } => {
            if collections
                .iter()
                .any(|collection| &collection.id == group_id)
            {
                Ok(())
            } else {
                Err(Failure::new(
                    ErrorCode::InvalidInput,
                    "outbound group is not present in the observed inventory",
                    false,
                ))
            }
        }
        OutboundUiPreferenceIntent::ReorderGroups { group_ids, .. } => {
            if group_ids
                .iter()
                .all(|id| collections.iter().any(|collection| &collection.id == id))
            {
                Ok(())
            } else {
                Err(Failure::new(
                    ErrorCode::InvalidInput,
                    "outbound group order contains an unobserved collection",
                    false,
                ))
            }
        }
        OutboundUiPreferenceIntent::SetSearchQuery(_)
        | OutboundUiPreferenceIntent::SetSortOrder(_)
        | OutboundUiPreferenceIntent::SetHealthFilter(_)
        | OutboundUiPreferenceIntent::SetLayout(_)
        | OutboundUiPreferenceIntent::ResetGroupOrder { .. } => Ok(()),
    }
}

fn require_probe_supported(availability: OutboundProbeAvailability) -> Result<(), Failure> {
    match availability {
        OutboundProbeAvailability::Supported => Ok(()),
        OutboundProbeAvailability::Unsupported => Err(Failure::unsupported(
            "outbound probing is unsupported for the requested outbound",
        )),
        OutboundProbeAvailability::Unknown => Err(Failure::new(
            ErrorCode::NotReady,
            "outbound probe availability has not been observed",
            true,
        )),
    }
}

fn verify_selection(
    snapshot: &OutboundInventorySnapshot,
    collection_id: &OutboundId,
    outbound_id: &OutboundId,
) -> Result<(), Failure> {
    let OutboundInventoryState::Ready { collections, .. } = snapshot.inventory() else {
        return Err(invalid_observation(
            "selection result did not include a ready inventory",
        ));
    };
    let collection = collections
        .iter()
        .find(|collection| &collection.id == collection_id)
        .ok_or_else(|| invalid_observation("selection result omitted its collection"))?;
    match &collection.selection {
        OutboundSelectionState::Ready { selected } if selected == outbound_id => Ok(()),
        OutboundSelectionState::Failed { failure } => Err(failure.clone()),
        OutboundSelectionState::Unsupported => Err(Failure::unsupported(
            "backend reports outbound selection as unsupported",
        )),
        OutboundSelectionState::Unknown
        | OutboundSelectionState::Empty
        | OutboundSelectionState::Ready { .. } => Err(invalid_observation(
            "selection readback did not confirm the requested outbound",
        )),
    }
}

pub(crate) fn verify_scope(expected: &CoreScope, observed: &CoreScope) -> Result<(), Failure> {
    if expected == observed {
        return Ok(());
    }
    Err(invalid_observation(
        "outbound observation belongs to a different core scope",
    ))
}

fn verify_newer_revision(
    expected: SnapshotRevision,
    observed: SnapshotRevision,
) -> Result<(), Failure> {
    if observed > expected {
        return Ok(());
    }
    Err(invalid_observation(
        "outbound operation returned a stale snapshot revision",
    ))
}

fn invalid_observation(message: impl Into<String>) -> Failure {
    Failure::new(ErrorCode::InvalidState, message, true)
}
