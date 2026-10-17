//! Mihomo adapter for engine-neutral outbound observations.

use crate::api::MihomoOutboundApi;
use crate::client::{MAX_MIHOMO_DELAY_TIMEOUT_MS, MihomoClient};
#[path = "outbound/mapping.rs"]
mod mapping;
use crate::error::MihomoError;
use crate::network_error::network_error;
use infiltrator_contract::capability::Capability;
use infiltrator_domain::proxy::{Proxy, ProxyHistory};
use infiltrator_ports::core_scope::{CoreScopeObservation, CoreScopeSource};
use infiltrator_ports::error::PortError;
use infiltrator_ports::outbound::{OutboundPort, OutboundProbeRequest, ProbeTarget};
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::{CoreScope, EngineId};
use lurker_core_contract::outbound::{
    details::{
        OutboundCollectionKind, OutboundDetail, OutboundDetailsSnapshot, OutboundDetailsState,
        OutboundEndpoint, OutboundFeature, OutboundHealthObservation, OutboundLatencySample,
        OutboundObservation, OutboundProbeAvailability, OutboundProtocolObservation,
        OutboundRttStatistics, OutboundSnapshot, OutboundUdpSupport,
    },
    identity::{OutboundCipherId, OutboundId, OutboundProtocolId, SnapshotRevision},
    inventory::{
        OutboundInventorySnapshot, OutboundInventoryState, OutboundObservationSnapshot,
        OutboundProbeSnapshot, OutboundProbeState, OutboundSelectionSet, OutboundSelectionState,
        SelectionAvailability,
    },
};
use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Mihomo outbound access serialized behind one owner-scoped observation fence.
///
/// Mihomo does not publish controller revisions or accept conditional writes.
/// This fence only covers calls made through this adapter instance; callers
/// that bypass it and write directly to the controller remain outside the fence.
pub struct MihomoOutboundAdapter {
    api: Arc<dyn MihomoOutboundApi>,
    scope_source: Arc<dyn CoreScopeSource>,
    observation: Mutex<OutboundObservationState>,
}

impl MihomoOutboundAdapter {
    pub fn new(api: Arc<MihomoClient>, scope_source: Arc<dyn CoreScopeSource>) -> Self {
        Self {
            api,
            scope_source,
            observation: Mutex::new(OutboundObservationState::default()),
        }
    }

    fn verify_scope(&self, requested: &CoreScope) -> Result<(), PortError> {
        match self.scope_source.scope_observation() {
            CoreScopeObservation::Observed(current) if &current == requested => {}
            CoreScopeObservation::Observed(_) => {
                return Err(rejected(
                    ErrorCode::InvalidState,
                    "active core scope changed",
                ));
            }
            CoreScopeObservation::Unavailable => {
                return Err(rejected(
                    ErrorCode::NotReady,
                    "no active core scope is available",
                ));
            }
        }
        if requested.identity.engine.as_str() != "mihomo" {
            return Err(PortError::unsupported(
                Capability::RoutingIntent,
                "Mihomo outbound adapter received a non-Mihomo core scope",
            ));
        }
        Ok(())
    }

    async fn read_observation(
        &self,
    ) -> Result<(OutboundInventoryState, OutboundDetailsState), PortError> {
        let proxies = self.api.get_proxies().await.map_err(network_error)?;
        let inventory = mapping::map_inventory(&proxies).map_err(PortError::Rejected)?;
        let details = mapping::map_details(&proxies).map_err(PortError::Rejected)?;
        Ok((inventory, details))
    }

    fn current_snapshot(
        state: &OutboundObservationState,
        scope: &CoreScope,
    ) -> Result<OutboundInventorySnapshot, PortError> {
        let inventory = state.inventory.as_ref().ok_or_else(|| {
            rejected(
                ErrorCode::NotReady,
                "outbound inventory has not been observed",
            )
        })?;
        OutboundInventorySnapshot::new(scope.clone(), state.inventory_revision, inventory.clone())
            .map_err(|error| PortError::Failed(error.to_string()))
    }

    fn current_details_snapshot(
        state: &OutboundObservationState,
        scope: &CoreScope,
    ) -> Result<OutboundDetailsSnapshot, PortError> {
        let details = state.details.as_ref().ok_or_else(|| {
            rejected(
                ErrorCode::NotReady,
                "outbound details have not been observed",
            )
        })?;
        OutboundDetailsSnapshot::new(scope.clone(), state.detail_revision, details.clone())
            .map_err(|error| PortError::Failed(error.to_string()))
    }

    fn current_observation(
        state: &OutboundObservationState,
        scope: &CoreScope,
    ) -> Result<OutboundObservationSnapshot, PortError> {
        OutboundObservationSnapshot::new(
            Self::current_snapshot(state, scope)?,
            Self::current_details_snapshot(state, scope)?,
        )
        .map_err(|error| PortError::Failed(error.to_string()))
    }

    async fn preflight_revision(
        &self,
        scope: &CoreScope,
        expected_revision: SnapshotRevision,
        state: &mut OutboundObservationState,
    ) -> Result<OutboundInventoryState, PortError> {
        self.verify_scope(scope)?;
        if state.scope.as_ref() != Some(scope) || state.inventory_revision != expected_revision {
            return Err(stale_revision());
        }
        if state.inventory.is_none() {
            return Err(stale_revision());
        }

        let current = match self.read_observation().await {
            Ok(observation) => observation,
            Err(error) => {
                let failure = Failure::from(error.clone());
                state.observe_failure(scope, failure)?;
                return Err(error);
            }
        };
        self.verify_scope(scope)?;
        let inventory_changed = state.inventory.as_ref() != Some(&current.0);
        state.observe(scope, current.0, current.1)?;
        if inventory_changed {
            return Err(stale_revision());
        }
        state.inventory.clone().ok_or_else(stale_revision)
    }
}

#[async_trait::async_trait]
impl OutboundPort for MihomoOutboundAdapter {
    async fn inventory(&self, scope: CoreScope) -> Result<OutboundInventorySnapshot, PortError> {
        self.verify_scope(&scope)?;
        let mut state = self.observation.lock().await;
        let (inventory, details) = match self.read_observation().await {
            Ok(observation) => observation,
            Err(error) => {
                let failure = Failure::from(error);
                (
                    OutboundInventoryState::Failed {
                        failure: failure.clone(),
                    },
                    OutboundDetailsState::Failed { failure },
                )
            }
        };
        self.verify_scope(&scope)?;
        state.observe(&scope, inventory, details)?;
        Self::current_snapshot(&state, &scope)
    }

    async fn observation(
        &self,
        scope: CoreScope,
    ) -> Result<OutboundObservationSnapshot, PortError> {
        self.verify_scope(&scope)?;
        let mut state = self.observation.lock().await;
        let (inventory, details) = match self.read_observation().await {
            Ok(observation) => observation,
            Err(error) => {
                let failure = Failure::from(error);
                (
                    OutboundInventoryState::Failed {
                        failure: failure.clone(),
                    },
                    OutboundDetailsState::Failed { failure },
                )
            }
        };
        self.verify_scope(&scope)?;
        state.observe(&scope, inventory, details)?;
        Self::current_observation(&state, &scope)
    }

    async fn details(
        &self,
        scope: CoreScope,
        expected_revision: SnapshotRevision,
    ) -> Result<OutboundDetailsSnapshot, PortError> {
        self.verify_scope(&scope)?;
        let mut state = self.observation.lock().await;
        if state.scope.as_ref() != Some(&scope) || state.inventory_revision != expected_revision {
            return Err(stale_revision());
        }
        let current = match self.read_observation().await {
            Ok(observation) => observation,
            Err(error) => {
                state.observe_failure(&scope, Failure::from(error.clone()))?;
                return Err(error);
            }
        };
        self.verify_scope(&scope)?;
        let inventory_changed = state.inventory.as_ref() != Some(&current.0);
        state.observe(&scope, current.0, current.1)?;
        if inventory_changed || state.inventory_revision != expected_revision {
            return Err(stale_revision());
        }
        Self::current_details_snapshot(&state, &scope)
    }

    async fn select(
        &self,
        scope: CoreScope,
        expected_revision: SnapshotRevision,
        collection_id: OutboundId,
        outbound_id: OutboundId,
    ) -> Result<OutboundInventorySnapshot, PortError> {
        self.verify_scope(&scope)?;
        verify_id_engine(&scope, &collection_id)?;
        verify_id_engine(&scope, &outbound_id)?;
        let mut state = self.observation.lock().await;
        let inventory = self
            .preflight_revision(&scope, expected_revision, &mut state)
            .await?;
        verify_selectable(&inventory, &collection_id, &outbound_id)?;
        self.verify_scope(&scope)?;

        let switch_result = self
            .api
            .switch_proxy(collection_id.value(), outbound_id.value())
            .await;
        let observed = match self.read_observation().await {
            Ok(observation) => observation,
            Err(error) => {
                state.observe_failure(&scope, Failure::from(error.clone()))?;
                return Err(error);
            }
        };
        self.verify_scope(&scope)?;
        state.observe(&scope, observed.0, observed.1)?;
        let snapshot = Self::current_snapshot(&state, &scope)?;
        if selected_member(snapshot.inventory(), &collection_id) == Some(&outbound_id) {
            return Ok(snapshot);
        }
        if let Err(error) = switch_result {
            return Err(network_error(error));
        }
        Err(rejected(
            ErrorCode::InvalidState,
            "Mihomo selection readback did not confirm the requested outbound",
        ))
    }

    async fn probe(
        &self,
        scope: CoreScope,
        expected_revision: SnapshotRevision,
        outbound_id: OutboundId,
        request: OutboundProbeRequest,
    ) -> Result<OutboundProbeSnapshot, PortError> {
        self.verify_scope(&scope)?;
        verify_id_engine(&scope, &outbound_id)?;
        let (target, timeout_ms) = mihomo_probe_values(&request)?;
        let mut state = self.observation.lock().await;
        let inventory = self
            .preflight_revision(&scope, expected_revision, &mut state)
            .await?;
        verify_probe_member(&inventory, &outbound_id)?;
        self.verify_scope(&scope)?;

        let probe_result = self
            .api
            .test_delay(outbound_id.value(), &target, timeout_ms)
            .await;
        let after = match self.read_observation().await {
            Ok(observation) => observation,
            Err(error) => {
                state.observe_failure(&scope, Failure::from(error.clone()))?;
                return Err(error);
            }
        };
        self.verify_scope(&scope)?;
        state.observe(&scope, after.0, after.1)?;
        state.bump_details(&scope)?;
        let probe_state = map_probe_result(probe_result);
        OutboundProbeSnapshot::new(
            scope,
            state.inventory_revision,
            state.detail_revision,
            outbound_id,
            probe_state,
        )
        .map_err(|error| PortError::Failed(error.to_string()))
    }
}

fn map_probe_result(result: Result<u32, MihomoError>) -> OutboundProbeState {
    match result {
        Ok(delay_ms) => OutboundProbeState::Measured { delay_ms },
        Err(MihomoError::Http(error)) if error.is_timeout() => OutboundProbeState::TimedOut,
        Err(error) => OutboundProbeState::Failed {
            failure: Failure::from(network_error(error)),
        },
    }
}

#[derive(Default)]
struct OutboundObservationState {
    scope: Option<CoreScope>,
    inventory_revision: SnapshotRevision,
    detail_revision: SnapshotRevision,
    inventory: Option<OutboundInventoryState>,
    details: Option<OutboundDetailsState>,
}

impl OutboundObservationState {
    fn observe(
        &mut self,
        scope: &CoreScope,
        inventory: OutboundInventoryState,
        details: OutboundDetailsState,
    ) -> Result<(), PortError> {
        if self.scope.as_ref() != Some(scope) {
            self.scope = Some(scope.clone());
            self.inventory_revision = SnapshotRevision::new(1);
            self.detail_revision = SnapshotRevision::new(1);
            self.inventory = Some(inventory);
            self.details = Some(details);
            return Ok(());
        }
        if self.inventory.as_ref() != Some(&inventory) {
            self.bump_inventory(scope)?;
            self.inventory = Some(inventory);
        }
        if self.details.as_ref() != Some(&details) {
            self.bump_details(scope)?;
            self.details = Some(details);
        }
        Ok(())
    }

    fn observe_failure(&mut self, scope: &CoreScope, failure: Failure) -> Result<(), PortError> {
        self.observe(
            scope,
            OutboundInventoryState::Failed {
                failure: failure.clone(),
            },
            OutboundDetailsState::Failed { failure },
        )
    }

    fn bump_inventory(&mut self, scope: &CoreScope) -> Result<(), PortError> {
        if self.scope.as_ref() != Some(scope) {
            self.scope = Some(scope.clone());
            self.inventory_revision = SnapshotRevision::new(1);
            self.inventory = None;
            self.details = None;
            self.detail_revision = SnapshotRevision::new(1);
            return Ok(());
        }
        let next = self
            .inventory_revision
            .value()
            .checked_add(1)
            .ok_or_else(|| PortError::Failed("outbound inventory revision exhausted".to_owned()))?;
        self.inventory_revision = SnapshotRevision::new(next);
        Ok(())
    }

    fn bump_details(&mut self, scope: &CoreScope) -> Result<(), PortError> {
        if self.scope.as_ref() != Some(scope) {
            self.scope = Some(scope.clone());
            self.inventory_revision = SnapshotRevision::new(1);
            self.detail_revision = SnapshotRevision::new(1);
            self.inventory = None;
            self.details = None;
            return Ok(());
        }
        let next =
            self.detail_revision.value().checked_add(1).ok_or_else(|| {
                PortError::Failed("outbound detail revision exhausted".to_owned())
            })?;
        self.detail_revision = SnapshotRevision::new(next);
        Ok(())
    }
}

fn verify_selectable(
    inventory: &OutboundInventoryState,
    collection_id: &OutboundId,
    outbound_id: &OutboundId,
) -> Result<(), PortError> {
    let OutboundInventoryState::Ready { collections, .. } = inventory else {
        return Err(rejected(
            ErrorCode::NotReady,
            "Mihomo outbound inventory is not ready",
        ));
    };
    let collection = collections
        .iter()
        .find(|collection| &collection.id == collection_id)
        .ok_or_else(|| {
            rejected(
                ErrorCode::InvalidInput,
                "outbound selector was not observed",
            )
        })?;
    if collection.availability != SelectionAvailability::Selectable {
        return Err(rejected(
            ErrorCode::Unsupported,
            "Mihomo only accepts selection for an observed Selector group",
        ));
    }
    if !collection.member_ids.contains(outbound_id) {
        return Err(rejected(
            ErrorCode::InvalidInput,
            "outbound is not in the observed selector",
        ));
    }
    Ok(())
}

fn verify_probe_member(
    inventory: &OutboundInventoryState,
    outbound_id: &OutboundId,
) -> Result<(), PortError> {
    let OutboundInventoryState::Ready { entries, .. } = inventory else {
        return Err(rejected(
            ErrorCode::NotReady,
            "Mihomo outbound inventory is not ready",
        ));
    };
    if entries.iter().any(|entry| &entry.id == outbound_id) {
        return Ok(());
    }
    Err(rejected(
        ErrorCode::InvalidInput,
        "outbound was not observed in the current inventory",
    ))
}

fn selected_member<'a>(
    inventory: &'a OutboundInventoryState,
    collection_id: &OutboundId,
) -> Option<&'a OutboundId> {
    let OutboundInventoryState::Ready { collections, .. } = inventory else {
        return None;
    };
    collections
        .iter()
        .find(|collection| &collection.id == collection_id)
        .and_then(|collection| match &collection.selection {
            OutboundSelectionState::Ready { selected } => Some(selected),
            _ => None,
        })
}

fn mihomo_probe_values(request: &OutboundProbeRequest) -> Result<(String, u32), PortError> {
    let target = ProbeTarget::http_url(request.target().as_url().to_owned())
        .map_err(|error| rejected(ErrorCode::InvalidInput, error.to_string()))?;
    let timeout_ms = u32::try_from(request.timeout_ms()).map_err(|_| {
        rejected(
            ErrorCode::InvalidInput,
            "Mihomo delay endpoint accepts a u32 timeout in milliseconds",
        )
    })?;
    if timeout_ms == 0 {
        return Err(rejected(
            ErrorCode::InvalidInput,
            "Mihomo delay timeout must be greater than zero",
        ));
    }
    if timeout_ms > MAX_MIHOMO_DELAY_TIMEOUT_MS {
        return Err(rejected(
            ErrorCode::InvalidInput,
            "Mihomo delay endpoint timeout exceeds its signed-16-bit millisecond limit",
        ));
    }
    Ok((target.as_url().to_owned(), timeout_ms))
}

fn verify_id_engine(scope: &CoreScope, outbound_id: &OutboundId) -> Result<(), PortError> {
    if outbound_id.engine_id() == &scope.identity.engine {
        return Ok(());
    }
    Err(rejected(
        ErrorCode::InvalidInput,
        "outbound ID engine differs from core scope",
    ))
}

fn stale_revision() -> PortError {
    rejected(
        ErrorCode::InvalidState,
        "outbound snapshot revision is stale; refresh inventory before retrying",
    )
}

fn rejected(code: ErrorCode, message: impl Into<String>) -> PortError {
    let retryable = matches!(
        &code,
        ErrorCode::InvalidState | ErrorCode::NotReady | ErrorCode::Network | ErrorCode::Internal
    );
    PortError::Rejected(Failure::new(code, message, retryable))
}

fn invalid_wire(message: impl Into<String>) -> Failure {
    Failure::new(
        ErrorCode::InvalidState,
        format!("invalid Mihomo outbound observation: {}", message.into()),
        false,
    )
}

#[cfg(test)]
#[path = "outbound_tests.rs"]
mod tests;
