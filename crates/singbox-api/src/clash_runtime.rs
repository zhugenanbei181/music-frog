//! Typed, CoreScope-fenced sing-box Clash API operations.

use crate::clash_api::{SingBoxClashApiClient, SingBoxCoreVersionProvider};
use infiltrator_contract::capability::Capability;
use infiltrator_ports::core_scope::{CoreScopeObservation, CoreScopeSource};
use infiltrator_ports::error::PortError;
use infiltrator_ports::outbound::{OutboundPort, OutboundProbeRequest};
use infiltrator_ports::routing_intent::RoutingIntentPort;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::{CoreScope, EngineId};
use lurker_core_contract::outbound::details::{
    OutboundDetailsSnapshot, OutboundDetailsState, OutboundSnapshot,
};
use lurker_core_contract::outbound::identity::{OutboundId, SnapshotRevision};
use lurker_core_contract::outbound::inventory::{
    OutboundInventorySnapshot, OutboundInventoryState, OutboundObservationSnapshot,
    OutboundProbeSnapshot, OutboundProbeState, OutboundSelectionSet, OutboundSelectionState,
    SelectionAvailability,
};
use lurker_core_contract::routing_intent::{
    EngineDefinedRoutingIntent, RoutingIntent, RoutingIntentExtensionId,
};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// The observed sing-box `/configs.mode` value remains engine-defined. sing-box
/// uses it to select rule `clash_mode` branches; it does not mean one of the
/// neutral routing policies. Writes remain disabled until their behavior is
/// verified against a real enabled `clash_mode` configuration.
pub struct SingBoxRoutingIntentAdapter {
    api: SingBoxClashApiClient,
    scope_source: Arc<dyn CoreScopeSource>,
    version_provider: SingBoxCoreVersionProvider,
}

impl SingBoxRoutingIntentAdapter {
    pub fn new(api: SingBoxClashApiClient, scope_source: Arc<dyn CoreScopeSource>) -> Self {
        Self {
            version_provider: SingBoxCoreVersionProvider::new(api.clone()),
            api,
            scope_source,
        }
    }
}

#[async_trait::async_trait]
impl RoutingIntentPort for SingBoxRoutingIntentAdapter {
    async fn observe(&self, scope: &CoreScope) -> Result<RoutingIntent, PortError> {
        verify_live_scope(&self.version_provider, self.scope_source.as_ref(), scope).await?;
        let config: ClashConfigResponse = self.api.get_json(&["configs"]).await?;
        verify_live_scope(&self.version_provider, self.scope_source.as_ref(), scope).await?;
        routing_intent_from_mode(&config)
    }

    async fn apply(
        &self,
        scope: &CoreScope,
        intent: RoutingIntent,
    ) -> Result<RoutingIntent, PortError> {
        verify_owner_scope(self.scope_source.as_ref(), scope)?;
        match intent {
            RoutingIntent::EngineDefined(extension) if extension.engine() != &engine_id() => {
                Err(rejected(
                    ErrorCode::InvalidState,
                    "engine-defined routing intent belongs to another engine",
                ))
            }
            RoutingIntent::FollowRules
            | RoutingIntent::RouteAllThroughProxy
            | RoutingIntent::BypassProxy
            | RoutingIntent::EngineDefined(_) => Err(PortError::unsupported(
                Capability::RoutingIntent,
                "sing-box /configs.mode selects clash_mode rule state and has no verified write mapping to the requested routing intent",
            )),
        }
    }
}

#[derive(Deserialize)]
struct ClashConfigResponse {
    mode: String,
    #[serde(rename = "mode-list", default)]
    mode_list: Vec<String>,
}

fn routing_intent_from_mode(config: &ClashConfigResponse) -> Result<RoutingIntent, PortError> {
    if config.mode.trim().is_empty() || !config.mode_list.iter().any(|mode| mode == &config.mode) {
        return Err(rejected(
            ErrorCode::InvalidState,
            "sing-box /configs returned a mode outside its reported mode-list",
        ));
    }
    let canonical = config.mode.to_ascii_lowercase();
    if !canonical
        .bytes()
        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"._-".contains(&byte))
    {
        return Err(rejected(
            ErrorCode::InvalidState,
            "sing-box /configs.mode cannot be represented by the engine-defined routing ID",
        ));
    }
    let extension_id = RoutingIntentExtensionId::new(format!("mode.{canonical}"))
        .map_err(|error| rejected(ErrorCode::InvalidState, error.to_string()))?;
    Ok(RoutingIntent::EngineDefined(
        EngineDefinedRoutingIntent::new(engine_id(), extension_id),
    ))
}

/// CoreScope-aware outbound inventory and selector adapter.
///
/// The controller has no revision/CAS endpoint. This adapter therefore
/// serializes its own operations, compares a fresh `/proxies` observation with
/// the expected local revision immediately before writes, verifies the active
/// scope/version around HTTP calls, and accepts a write only after readback.
pub struct SingBoxOutboundAdapter {
    api: SingBoxClashApiClient,
    scope_source: Arc<dyn CoreScopeSource>,
    version_provider: SingBoxCoreVersionProvider,
    observation: Mutex<OutboundObservationState>,
}

impl SingBoxOutboundAdapter {
    pub fn new(api: SingBoxClashApiClient, scope_source: Arc<dyn CoreScopeSource>) -> Self {
        Self {
            version_provider: SingBoxCoreVersionProvider::new(api.clone()),
            api,
            scope_source,
            observation: Mutex::new(OutboundObservationState::default()),
        }
    }

    async fn read_inventory(&self) -> Result<OutboundInventoryState, PortError> {
        let response: ClashProxiesResponse = self.api.get_json(&["proxies"]).await?;
        map_inventory(response)
    }

    async fn preflight_revision(
        &self,
        scope: &CoreScope,
        expected_revision: SnapshotRevision,
        state: &mut OutboundObservationState,
    ) -> Result<OutboundInventoryState, PortError> {
        verify_live_scope(&self.version_provider, self.scope_source.as_ref(), scope).await?;
        let cached = match &state.state {
            OutboundObservationPhase::Ready {
                scope: observed_scope,
                revision,
                inventory,
            } if observed_scope == scope && *revision == expected_revision => inventory.clone(),
            OutboundObservationPhase::Ready { .. } | OutboundObservationPhase::Unobserved => {
                return Err(stale_revision());
            }
        };
        let current = self.read_inventory().await?;
        verify_live_scope(&self.version_provider, self.scope_source.as_ref(), scope).await?;
        state.observe(scope, current)?;
        let current = current_inventory(state, scope)?;
        if current.inventory() != &cached || current.revision() != expected_revision {
            return Err(stale_revision());
        }
        Ok(current.inventory().clone())
    }
}

#[async_trait::async_trait]
impl OutboundPort for SingBoxOutboundAdapter {
    async fn inventory(&self, scope: CoreScope) -> Result<OutboundInventorySnapshot, PortError> {
        verify_live_scope(&self.version_provider, self.scope_source.as_ref(), &scope).await?;
        let mut state = self.observation.lock().await;
        let inventory = self.read_inventory().await?;
        verify_live_scope(&self.version_provider, self.scope_source.as_ref(), &scope).await?;
        state.observe(&scope, inventory)?;
        current_inventory(&state, &scope)
    }

    async fn observation(
        &self,
        scope: CoreScope,
    ) -> Result<OutboundObservationSnapshot, PortError> {
        let inventory = self.inventory(scope.clone()).await?;
        let details = OutboundDetailsSnapshot::new(
            scope,
            SnapshotRevision::new(1),
            OutboundDetailsState::Unsupported,
        )
        .map_err(|error| PortError::Failed(error.to_string()))?;
        OutboundObservationSnapshot::new(inventory, details)
            .map_err(|error| PortError::Failed(error.to_string()))
    }

    async fn details(
        &self,
        scope: CoreScope,
        expected_revision: SnapshotRevision,
    ) -> Result<OutboundDetailsSnapshot, PortError> {
        let mut state = self.observation.lock().await;
        self.preflight_revision(&scope, expected_revision, &mut state)
            .await?;
        OutboundDetailsSnapshot::new(
            scope,
            SnapshotRevision::new(1),
            OutboundDetailsState::Unsupported,
        )
        .map_err(|error| PortError::Failed(error.to_string()))
    }

    async fn select(
        &self,
        scope: CoreScope,
        expected_revision: SnapshotRevision,
        collection_id: OutboundId,
        outbound_id: OutboundId,
    ) -> Result<OutboundInventorySnapshot, PortError> {
        verify_id_engine(&scope, &collection_id)?;
        verify_id_engine(&scope, &outbound_id)?;
        let mut state = self.observation.lock().await;
        let inventory = self
            .preflight_revision(&scope, expected_revision, &mut state)
            .await?;
        verify_selectable_member(&inventory, &collection_id, &outbound_id)?;
        verify_owner_scope(self.scope_source.as_ref(), &scope)?;

        let put_result = self
            .api
            .put_json(
                &["proxies", collection_id.value()],
                serde_json::json!({"name": outbound_id.value()}),
            )
            .await;
        let observed = self.read_inventory().await;
        verify_live_scope(&self.version_provider, self.scope_source.as_ref(), &scope).await?;
        let observed = observed?;
        state.observe(&scope, observed)?;
        let snapshot = current_inventory(&state, &scope)?;
        if selected_member(snapshot.inventory(), &collection_id) == Some(&outbound_id) {
            return Ok(snapshot);
        }
        if let Err(error) = put_result {
            return Err(error);
        }
        Err(rejected(
            ErrorCode::InvalidState,
            "sing-box selector readback did not confirm the requested outbound",
        ))
    }

    async fn probe(
        &self,
        scope: CoreScope,
        expected_revision: SnapshotRevision,
        outbound_id: OutboundId,
        _request: OutboundProbeRequest,
    ) -> Result<OutboundProbeSnapshot, PortError> {
        verify_id_engine(&scope, &outbound_id)?;
        let mut state = self.observation.lock().await;
        let inventory = self
            .preflight_revision(&scope, expected_revision, &mut state)
            .await?;
        if !inventory_contains(&inventory, &outbound_id) {
            return Err(rejected(
                ErrorCode::InvalidState,
                "probe outbound is absent from the observed sing-box inventory",
            ));
        }
        let current = current_inventory(&state, &scope)?;
        if current.revision() != expected_revision {
            return Err(stale_revision());
        }
        OutboundProbeSnapshot::new(
            scope,
            expected_revision,
            SnapshotRevision::new(1),
            outbound_id,
            OutboundProbeState::Unsupported,
        )
        .map_err(|error| PortError::Failed(error.to_string()))
    }
}

#[derive(Default)]
struct OutboundObservationState {
    state: OutboundObservationPhase,
}

#[derive(Default)]
enum OutboundObservationPhase {
    #[default]
    Unobserved,
    Ready {
        scope: CoreScope,
        revision: SnapshotRevision,
        inventory: OutboundInventoryState,
    },
}

impl OutboundObservationState {
    fn observe(
        &mut self,
        scope: &CoreScope,
        inventory: OutboundInventoryState,
    ) -> Result<(), PortError> {
        self.state = match &self.state {
            OutboundObservationPhase::Unobserved => OutboundObservationPhase::Ready {
                scope: scope.clone(),
                revision: SnapshotRevision::new(1),
                inventory,
            },
            OutboundObservationPhase::Ready {
                scope: previous_scope,
                ..
            } if previous_scope != scope => OutboundObservationPhase::Ready {
                scope: scope.clone(),
                revision: SnapshotRevision::new(1),
                inventory,
            },
            OutboundObservationPhase::Ready {
                scope: previous_scope,
                revision,
                inventory: previous_inventory,
            } if previous_inventory != &inventory => OutboundObservationPhase::Ready {
                scope: previous_scope.clone(),
                revision: SnapshotRevision::new(revision.value().checked_add(1).ok_or_else(
                    || PortError::Failed("sing-box inventory revision exhausted".to_owned()),
                )?),
                inventory,
            },
            OutboundObservationPhase::Ready {
                scope: previous_scope,
                revision,
                ..
            } => OutboundObservationPhase::Ready {
                scope: previous_scope.clone(),
                revision: *revision,
                inventory,
            },
        };
        Ok(())
    }
}

fn current_inventory(
    state: &OutboundObservationState,
    scope: &CoreScope,
) -> Result<OutboundInventorySnapshot, PortError> {
    match &state.state {
        OutboundObservationPhase::Ready {
            scope: observed_scope,
            revision,
            inventory,
        } if observed_scope == scope => {
            OutboundInventorySnapshot::new(scope.clone(), *revision, inventory.clone())
                .map_err(|error| PortError::Failed(error.to_string()))
        }
        OutboundObservationPhase::Ready { .. } => Err(stale_revision()),
        OutboundObservationPhase::Unobserved => Err(rejected(
            ErrorCode::NotReady,
            "sing-box outbound inventory has not been observed",
        )),
    }
}

#[derive(Deserialize)]
struct ClashProxiesResponse {
    proxies: BTreeMap<String, ClashProxy>,
}

#[derive(Deserialize)]
struct ClashProxy {
    #[serde(rename = "type")]
    proxy_type: String,
    name: String,
    #[serde(default)]
    now: Option<String>,
    #[serde(default)]
    all: Option<Vec<String>>,
}

fn map_inventory(response: ClashProxiesResponse) -> Result<OutboundInventoryState, PortError> {
    let engine = engine_id();
    let mut proxies = Vec::new();
    for (key, proxy) in response.proxies {
        // sing-box adds this synthetic compatibility collection to the API.
        // It is not an outbound from the configured core.
        if key == "GLOBAL" && proxy.name == "GLOBAL" && proxy.proxy_type == "Fallback" {
            continue;
        }
        if key != proxy.name {
            return Err(rejected(
                ErrorCode::InvalidState,
                "sing-box /proxies key did not match its reported proxy name",
            ));
        }
        proxies.push((proxy.proxy_type, proxy.name, proxy.now, proxy.all));
    }

    if proxies.is_empty() {
        return Ok(OutboundInventoryState::Empty);
    }

    let mut entries = Vec::with_capacity(proxies.len());
    let mut ids = BTreeMap::new();
    for (_, name, _, _) in &proxies {
        let id = OutboundId::new(engine.clone(), name.clone())
            .map_err(|error| rejected(ErrorCode::InvalidState, error.to_string()))?;
        ids.insert(name.clone(), id.clone());
        entries.push(OutboundSnapshot {
            id,
            display_name: Some(name.clone()),
        });
    }

    let mut collections = Vec::new();
    for (proxy_type, name, now, all) in proxies {
        let is_known_group = matches!(proxy_type.as_str(), "Selector" | "URLTest");
        if !is_known_group && all.is_none() && now.is_none() {
            continue;
        }
        let members = match all {
            Some(names) => names
                .iter()
                .map(|member| {
                    ids.get(member).cloned().ok_or_else(|| {
                        rejected(
                            ErrorCode::InvalidState,
                            format!("sing-box selector references missing outbound {member}"),
                        )
                    })
                })
                .collect::<Result<Vec<_>, _>>()?,
            None => Vec::new(),
        };
        let collection_id = ids
            .get(&name)
            .cloned()
            .ok_or_else(|| rejected(ErrorCode::InvalidState, "sing-box group ID was lost"))?;
        let selected = match now {
            Some(selected) => ids.get(&selected).cloned().map(|selected| {
                if members.contains(&selected) {
                    OutboundSelectionState::Ready { selected }
                } else {
                    OutboundSelectionState::Unknown
                }
            }),
            None => None,
        }
        .unwrap_or(OutboundSelectionState::Unknown);
        let availability = match proxy_type.as_str() {
            "Selector"
                if !members.is_empty()
                    && matches!(&selected, OutboundSelectionState::Ready { .. }) =>
            {
                SelectionAvailability::Selectable
            }
            "URLTest" => SelectionAvailability::Automatic,
            "Selector" => SelectionAvailability::Unknown,
            _ => SelectionAvailability::Unknown,
        };
        collections.push(OutboundSelectionSet {
            id: collection_id,
            member_ids: members,
            availability,
            selection: selected,
        });
    }

    Ok(OutboundInventoryState::Ready {
        entries,
        collections,
    })
}

fn verify_selectable_member(
    inventory: &OutboundInventoryState,
    collection_id: &OutboundId,
    outbound_id: &OutboundId,
) -> Result<(), PortError> {
    let OutboundInventoryState::Ready { collections, .. } = inventory else {
        return Err(rejected(
            ErrorCode::NotReady,
            "sing-box inventory is not ready",
        ));
    };
    let collection = collections
        .iter()
        .find(|collection| &collection.id == collection_id)
        .ok_or_else(|| {
            rejected(
                ErrorCode::InvalidState,
                "sing-box selector was not observed",
            )
        })?;
    if collection.availability != SelectionAvailability::Selectable {
        return Err(PortError::Rejected(Failure::unsupported(
            "sing-box only exposes selection writes for observed Selector groups",
        )));
    }
    if !collection.member_ids.contains(outbound_id) {
        return Err(rejected(
            ErrorCode::InvalidState,
            "requested outbound is not a member of the observed sing-box selector",
        ));
    }
    Ok(())
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
            OutboundSelectionState::Unknown
            | OutboundSelectionState::Empty
            | OutboundSelectionState::Unsupported
            | OutboundSelectionState::Failed { .. } => None,
        })
}

fn inventory_contains(inventory: &OutboundInventoryState, outbound_id: &OutboundId) -> bool {
    match inventory {
        OutboundInventoryState::Ready { entries, .. } => {
            entries.iter().any(|entry| &entry.id == outbound_id)
        }
        OutboundInventoryState::Unknown
        | OutboundInventoryState::Empty
        | OutboundInventoryState::Unsupported
        | OutboundInventoryState::Failed { .. } => false,
    }
}

fn verify_id_engine(scope: &CoreScope, id: &OutboundId) -> Result<(), PortError> {
    if id.engine_id() == &scope.identity.engine && id.engine_id() == &engine_id() {
        Ok(())
    } else {
        Err(rejected(
            ErrorCode::InvalidState,
            "sing-box outbound request used a foreign engine ID",
        ))
    }
}

fn verify_owner_scope(
    scope_source: &dyn CoreScopeSource,
    requested: &CoreScope,
) -> Result<(), PortError> {
    if requested.identity.engine != engine_id() {
        return Err(rejected(
            ErrorCode::InvalidState,
            "sing-box Clash API adapter received a scope for another engine",
        ));
    }
    if requested.identity.version.is_none() {
        return Err(rejected(
            ErrorCode::NotReady,
            "sing-box Clash API operations require an observed core version in CoreScope",
        ));
    }
    match scope_source.scope_observation() {
        CoreScopeObservation::Observed(current) if &current == requested => Ok(()),
        CoreScopeObservation::Observed(_) => Err(rejected(
            ErrorCode::InvalidState,
            "active CoreScope changed during sing-box Clash API operation",
        )),
        CoreScopeObservation::Unavailable => Err(rejected(
            ErrorCode::NotReady,
            "no active CoreScope is available for sing-box Clash API operation",
        )),
    }
}

async fn verify_live_scope(
    version_provider: &SingBoxCoreVersionProvider,
    scope_source: &dyn CoreScopeSource,
    requested: &CoreScope,
) -> Result<(), PortError> {
    verify_owner_scope(scope_source, requested)?;
    let actual = version_provider.observe().await?;
    verify_owner_scope(scope_source, requested)?;
    if requested.identity.version.as_ref() != Some(&actual) {
        return Err(rejected(
            ErrorCode::InvalidState,
            "live sing-box version differs from the observed CoreScope version",
        ));
    }
    Ok(())
}

fn stale_revision() -> PortError {
    rejected(
        ErrorCode::InvalidState,
        "sing-box outbound inventory revision is stale",
    )
}

fn rejected(code: ErrorCode, message: impl Into<String>) -> PortError {
    PortError::Rejected(Failure::new(code, message, false))
}

fn engine_id() -> EngineId {
    EngineId::sing_box()
}
