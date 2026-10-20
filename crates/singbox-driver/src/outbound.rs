//! CoreScope-fenced sing-box outbound inventory, selector switching and probe.

use crate::api::clash_api::{SingBoxClashApiClient, SingBoxCoreVersionProvider};
use crate::api::delay::{DelayAnswer, MAX_SINGBOX_DELAY_TIMEOUT_MS, refusal_code};
use crate::api::scope_fence::{engine_id, rejected, verify_live_scope, verify_owner_scope};
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::CoreScope;
use lurker_core_contract::outbound::details::{
    OutboundDetailsSnapshot, OutboundDetailsState, OutboundSnapshot,
};
use lurker_core_contract::outbound::identity::{OutboundId, SnapshotRevision};
use lurker_core_contract::outbound::inventory::{
    OutboundInventorySnapshot, OutboundInventoryState, OutboundObservationSnapshot,
    OutboundProbeSnapshot, OutboundProbeState, OutboundSelectionSet, OutboundSelectionState,
    SelectionAvailability,
};
use lurker_core_ports::core_scope::CoreScopeSource;
use lurker_core_ports::error::PortError;
use lurker_core_ports::outbound::{OutboundPort, OutboundProbeRequest, ProbeTarget};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

/// The details revision the sing-box driver publishes.
///
/// The sing-box Clash API exposes no neutral outbound details, so the driver
/// publishes the same constant revision its details snapshot carries. There
/// is no second details observation that could move it.
const DETAILS_REVISION: SnapshotRevision = SnapshotRevision::new(1);

/// The grace the driver adds to the engine's own delay budget.
///
/// The engine enforces the delay budget itself and answers its own timeout,
/// so the driver's deadline only covers the engine's answer: it is the
/// engine's budget plus this grace. A driver deadline that expires means the
/// engine did not answer in time, never that the measurement failed.
const DELAY_DEADLINE_GRACE: Duration = Duration::from_secs(2);

/// The slack the transport keeps beyond the driver's deadline.
///
/// The transport timeout stays strictly behind the driver's deadline, so the
/// driver's own deadline is the single authority over a late answer and the
/// typed outcome never depends on which of the two fired first.
const DELAY_TRANSPORT_SLACK: Duration = Duration::from_secs(1);

/// CoreScope-aware outbound inventory and selector adapter.
///
/// The controller has no revision/CAS endpoint. This adapter therefore
/// serializes its own operations, compares a fresh `/proxies` observation with
/// the expected local revision immediately before writes, verifies the active
/// scope/version around HTTP calls, and accepts a write only after readback.
///
/// The probe asks the engine's own delay endpoint for the measured latency of
/// one outbound and reports the engine's typed answer: a measurement, the
/// engine's timeout, a typed failure or the unsupported capability. The driver
/// never invents a measurement the engine did not publish.
pub struct SingBoxOutboundAdapter {
    api: SingBoxClashApiClient,
    scope_source: Arc<dyn CoreScopeSource>,
    version_provider: SingBoxCoreVersionProvider,
    observation: Mutex<OutboundObservationState>,
}

impl SingBoxOutboundAdapter {
    /// Creates the outbound adapter over one API client and scope source.
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

    /// Asks the engine for the delay of one outbound within the probe budget.
    ///
    /// The engine enforces the delay budget itself, so the driver's deadline
    /// only covers the engine's answer. A deadline that expires is a timeout
    /// of the probe, never a measurement and never a failure of the engine.
    async fn measure_delay(
        &self,
        outbound_id: &OutboundId,
        target: &str,
        timeout_ms: u32,
    ) -> Result<DelayAnswer, PortError> {
        let deadline = Duration::from_millis(u64::from(timeout_ms)) + DELAY_DEADLINE_GRACE;
        let transport_timeout = deadline + DELAY_TRANSPORT_SLACK;
        match tokio::time::timeout(
            deadline,
            self.api
                .get_delay(outbound_id.value(), target, timeout_ms, transport_timeout),
        )
        .await
        {
            Ok(result) => result,
            Err(_elapsed) => Ok(DelayAnswer::TimedOut),
        }
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
            DETAILS_REVISION,
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
        OutboundDetailsSnapshot::new(scope, DETAILS_REVISION, OutboundDetailsState::Unsupported)
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
        put_result?;
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
        request: OutboundProbeRequest,
    ) -> Result<OutboundProbeSnapshot, PortError> {
        verify_id_engine(&scope, &outbound_id)?;
        let (target, timeout_ms) = singbox_probe_values(&request)?;
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

        // The preflight ends with a live-scope check, so the engine is still
        // the observed core when the delay request leaves the driver.
        let answer = self.measure_delay(&outbound_id, &target, timeout_ms).await;

        // The probe is fenced like every other write: the inventory is read
        // again afterwards, so the snapshot carries the revision the engine
        // published while the probe ran.
        let observed = self.read_inventory().await;
        verify_live_scope(&self.version_provider, self.scope_source.as_ref(), &scope).await?;
        let observed = observed?;
        state.observe(&scope, observed)?;
        let current = current_inventory(&state, &scope)?;

        let probe_state = match answer {
            Ok(answer) => map_delay_answer(answer),
            Err(error) => OutboundProbeState::Failed {
                failure: Failure::from(error),
            },
        };
        OutboundProbeSnapshot::new(
            scope,
            current.revision(),
            DETAILS_REVISION,
            outbound_id,
            probe_state,
        )
        .map_err(|error| PortError::Failed(error.to_string()))
    }
}

/// Maps the engine's typed delay answer onto the neutral probe state.
///
/// Every answer keeps its own outcome: a measurement is a measurement, the
/// engine's own timeout is a timeout, a refusal is a typed failure and an
/// answer the engine's delay endpoint does not produce is the capability
/// being unsupported. No answer is ever turned into another one.
fn map_delay_answer(answer: DelayAnswer) -> OutboundProbeState {
    match answer {
        DelayAnswer::Measured { delay_ms } => OutboundProbeState::Measured { delay_ms },
        DelayAnswer::TimedOut => OutboundProbeState::TimedOut,
        DelayAnswer::Refused { status, reason } => OutboundProbeState::Failed {
            failure: refusal_failure(status, reason),
        },
        DelayAnswer::NotProvided => OutboundProbeState::Unsupported,
        DelayAnswer::Malformed => OutboundProbeState::Failed {
            failure: Failure::new(
                ErrorCode::InvalidState,
                "the sing-box delay endpoint answered with a body that is not a measurement",
                false,
            ),
        },
    }
}

/// Builds the neutral failure of one refused delay answer.
fn refusal_failure(status: u16, reason: Option<String>) -> Failure {
    let message = match reason {
        Some(reason) => {
            format!("the sing-box delay endpoint refused the request with HTTP {status}: {reason}")
        }
        None => format!("the sing-box delay endpoint refused the request with HTTP {status}"),
    };
    Failure::new(refusal_code(status), message, false)
}

/// The probe values the sing-box delay endpoint accepts.
///
/// The endpoint carries the timeout as a signed 16-bit millisecond count, so
/// a timeout it cannot carry is refused before the request is sent.
fn singbox_probe_values(request: &OutboundProbeRequest) -> Result<(String, u32), PortError> {
    let target = ProbeTarget::http_url(request.target().as_url().to_owned())
        .map_err(|error| rejected(ErrorCode::InvalidInput, error.to_string()))?;
    let timeout_ms = u32::try_from(request.timeout_ms()).map_err(|_| {
        rejected(
            ErrorCode::InvalidInput,
            "the sing-box delay endpoint carries the timeout as a millisecond count",
        )
    })?;
    if timeout_ms == 0 {
        return Err(rejected(
            ErrorCode::InvalidInput,
            "the sing-box delay timeout must be greater than zero",
        ));
    }
    if timeout_ms > MAX_SINGBOX_DELAY_TIMEOUT_MS {
        return Err(rejected(
            ErrorCode::InvalidInput,
            "the sing-box delay timeout exceeds the endpoint's signed 16-bit millisecond limit",
        ));
    }
    Ok((target.as_url().to_owned(), timeout_ms))
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

fn stale_revision() -> PortError {
    rejected(
        ErrorCode::InvalidState,
        "sing-box outbound inventory revision is stale",
    )
}

#[cfg(test)]
#[path = "outbound_tests.rs"]
mod tests;
