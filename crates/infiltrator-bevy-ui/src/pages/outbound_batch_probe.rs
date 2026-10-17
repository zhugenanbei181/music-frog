//! Bevy's request builder and typed readback for observed outbound batches.

use crate::command::{CommandSinkHandle, UiCommand};
use crate::command_events::CommandExecutedEvent;
use crate::route::{ActiveRoute, Route};
use crate::surface::LatestSurfaceSnapshot;
use bevy::ecs::observer::On;
use bevy::ecs::resource::Resource;
use bevy::ecs::system;
use bevy::ecs::system::ResMut;
use infiltrator_contract::command::RequestId;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::core_capabilities::{
    CapabilityAccess, CoreCapabilityStatus, CoreOperation, OutboundOperation,
};
use infiltrator_contract::runtime_settings::RuntimeProbeSettingsState;
use infiltrator_contract::surface_snapshot::{PageStatus, SurfaceSnapshot};
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::CoreScope;
use lurker_core_contract::outbound::{
    inventory::OutboundInventoryState,
    probe::{
        OutboundProbeBatchScope, OutboundProbeBatchSnapshot, OutboundProbeConcurrency,
        ProbeOutboundBatchRequest,
    },
};
use std::collections::HashMap;
use std::collections::HashSet;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingOutboundBatchProbe {
    pub request: ProbeOutboundBatchRequest,
    pub scope: CoreScope,
    pub owner_route: Route,
}

/// Typed command feedback retained independently of a page's mount lifetime.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct OutboundBatchProbeReadback {
    pub pending: HashMap<RequestId, PendingOutboundBatchProbe>,
    pub last_request_id: Option<RequestId>,
    pub last_result: Option<OutboundProbeBatchSnapshot>,
    pub last_failure: Option<Failure>,
}

impl OutboundBatchProbeReadback {
    fn track(
        &mut self,
        request_id: RequestId,
        request: ProbeOutboundBatchRequest,
        scope: CoreScope,
        owner_route: Route,
    ) {
        self.pending.insert(
            request_id,
            PendingOutboundBatchProbe {
                request,
                scope,
                owner_route,
            },
        );
        self.last_request_id = Some(request_id);
        self.last_result = None;
        self.last_failure = None;
    }
}

/// Submit one batch against the latest accepted engine-neutral snapshot.
/// Collection requests preserve the opaque collection ID; the Core owner
/// expands its direct members under the exact observed inventory revision.
pub fn submit_batch(
    snapshot: &SurfaceSnapshot,
    scope: OutboundProbeBatchScope,
    owner_route: Route,
    sink: &CommandSinkHandle,
    readback: &mut OutboundBatchProbeReadback,
) -> bool {
    let request = match request_from_snapshot(snapshot, scope) {
        Ok(request) => request,
        Err(failure) => {
            readback.last_failure = Some(failure);
            readback.last_result = None;
            return false;
        }
    };
    let Some(core_scope) = snapshot.core.scope() else {
        readback.last_failure = Some(not_ready("active Core scope is unavailable"));
        return false;
    };
    let Some(request_id) = sink.submit_tracked(UiCommand::ProbeOutboundBatch {
        request: request.clone(),
    }) else {
        readback.last_failure = Some(not_ready(
            "outbound batch probe sink supplies no terminal acknowledgment",
        ));
        return false;
    };
    readback.track(request_id, request, core_scope, owner_route);
    true
}

pub fn request_from_snapshot(
    snapshot: &SurfaceSnapshot,
    target_scope: OutboundProbeBatchScope,
) -> Result<ProbeOutboundBatchRequest, Failure> {
    let Some(scope) = snapshot.core.scope() else {
        return Err(not_ready("active Core scope is unavailable"));
    };
    let inventory_page = &snapshot.outbound_inventory;
    if !successful_page(&inventory_page.status) {
        return Err(not_ready("outbound inventory is not observed"));
    }
    let Some(inventory) = inventory_page.data.as_ref() else {
        return Err(not_ready("outbound inventory observation has no data"));
    };
    if inventory.scope() != &scope {
        return Err(not_ready(
            "outbound inventory belongs to another Core scope",
        ));
    }

    let details_page = &snapshot.outbound_details;
    if !successful_page(&details_page.status) {
        return Err(not_ready("outbound details are not observed"));
    }
    let Some(details) = details_page.data.as_ref() else {
        return Err(not_ready("outbound details observation has no data"));
    };
    if details.scope() != &scope {
        return Err(not_ready("outbound details belong to another Core scope"));
    }

    match (inventory.inventory(), &target_scope) {
        (OutboundInventoryState::Ready { entries, .. }, OutboundProbeBatchScope::All) => {
            if entries
                .iter()
                .any(|entry| details.detail(&entry.id).is_none())
            {
                return Err(not_ready(
                    "outbound details do not cover the accepted inventory",
                ));
            }
        }
        (
            OutboundInventoryState::Ready {
                entries,
                collections,
            },
            OutboundProbeBatchScope::Collection(id),
        ) => {
            let Some(collection) = collections.iter().find(|collection| &collection.id == id)
            else {
                return Err(not_ready("outbound collection is no longer observed"));
            };
            if collection.member_ids.iter().any(|member| {
                !entries.iter().any(|entry| &entry.id == member) || details.detail(member).is_none()
            }) {
                return Err(not_ready(
                    "outbound details do not cover the collection's direct members",
                ));
            }
        }
        (OutboundInventoryState::Ready { entries, .. }, OutboundProbeBatchScope::One(id)) => {
            if !entries.iter().any(|entry| &entry.id == id) || details.detail(id).is_none() {
                return Err(not_ready("outbound is no longer observed"));
            }
        }
        (
            OutboundInventoryState::Ready {
                entries,
                collections,
            },
            OutboundProbeBatchScope::Selected(ids),
        ) => {
            let mut unique = HashSet::new();
            for id in ids {
                if !unique.insert(id) {
                    return Err(invalid_input(
                        "selected outbound targets contain a duplicate",
                    ));
                }
                if collections.iter().any(|collection| &collection.id == id) {
                    return Err(invalid_input(
                        "a selected outbound probe target is a collection",
                    ));
                }
                if !entries.iter().any(|entry| &entry.id == id) || details.detail(id).is_none() {
                    return Err(not_ready("a selected outbound is no longer observed"));
                }
            }
        }
        (OutboundInventoryState::Empty, OutboundProbeBatchScope::All) => {}
        (OutboundInventoryState::Empty, _) => {
            return Err(not_ready("outbound inventory is empty"));
        }
        _ => return Err(not_ready("outbound inventory is not ready")),
    }

    target_scope
        .validate_scope(&scope)
        .map_err(|error| invalid_input(error.to_string()))?;
    if !probe_capability_available(snapshot, &scope) {
        return Err(Failure::unsupported(
            "active Core capability does not support outbound probing",
        ));
    }
    let settings = match &snapshot.probe_settings {
        RuntimeProbeSettingsState::Available { settings } => settings,
        RuntimeProbeSettingsState::Unavailable { failure } => return Err(failure.clone()),
    };
    let concurrency = OutboundProbeConcurrency::new(snapshot.speedtest.concurrency.get())
        .map_err(|error| invalid_input(error.to_string()))?;
    ProbeOutboundBatchRequest::new(
        target_scope,
        inventory.revision(),
        settings.url(),
        u64::from(settings.timeout_ms()),
        concurrency,
    )
    .map_err(|error| invalid_input(error.to_string()))
}

pub fn finish_batch_probe(
    event: On<CommandExecutedEvent>,
    latest: Option<system::Res<LatestSurfaceSnapshot>>,
    active_route: Option<system::Res<ActiveRoute>>,
    mut readback: ResMut<OutboundBatchProbeReadback>,
) {
    let Some(pending) = readback.pending.remove(&event.request_id) else {
        return;
    };
    if readback.last_request_id != Some(event.request_id)
        || active_route.as_deref().and_then(|route| route.0) != Some(pending.owner_route)
        || latest
            .as_deref()
            .and_then(|snapshot| snapshot.0.core.scope())
            .as_ref()
            != Some(&pending.scope)
        || latest.as_deref().is_none_or(|snapshot| {
            !successful_page(&snapshot.0.outbound_inventory.status)
                || snapshot
                    .0
                    .outbound_inventory
                    .data
                    .as_ref()
                    .is_none_or(|inventory| {
                        inventory.revision() != pending.request.expected_inventory_revision()
                            || inventory.scope() != &pending.scope
                    })
        })
    {
        return;
    }
    if !matches!(
        &event.command,
        UiCommand::ProbeOutboundBatch { request } if request == &pending.request
    ) {
        readback.last_failure = Some(invalid_state(
            "outbound batch receipt did not match its submitted request",
        ));
        readback.last_result = None;
        return;
    }
    match &event.result {
        Err(failure) => {
            readback.last_failure = Some(failure.clone());
            readback.last_result = None;
        }
        Ok(CommandOutput::OutboundProbeBatch(result))
            if result.is_confirmed_by(&pending.request) && result.scope() == &pending.scope =>
        {
            readback.last_failure = None;
            readback.last_result = Some(result.clone());
        }
        Ok(_) => {
            readback.last_failure = Some(invalid_state(
                "outbound batch receipt returned an unrelated or unconfirmed result",
            ));
            readback.last_result = None;
        }
    }
}

fn successful_page(status: &PageStatus) -> bool {
    matches!(status, PageStatus::Ready | PageStatus::Empty)
}

fn probe_capability_available(snapshot: &SurfaceSnapshot, scope: &CoreScope) -> bool {
    if snapshot.core_capability_snapshot.status != PageStatus::Ready {
        return false;
    }
    let Some(capabilities) = snapshot.core_capability_snapshot.data.as_ref() else {
        return false;
    };
    if capabilities.scope.engine_id != scope.identity.engine
        || capabilities.scope.core_version != scope.identity.version
    {
        return false;
    }
    capabilities
        .resolution(CoreOperation::Outbound(OutboundOperation::Probe))
        .is_some_and(|resolution| {
            matches!(
                &resolution.status,
                CoreCapabilityStatus::Available {
                    access: CapabilityAccess::Supported,
                    ..
                }
            )
        })
}

fn not_ready(message: impl Into<String>) -> Failure {
    Failure::new(ErrorCode::NotReady, message, true)
}

fn invalid_input(message: impl Into<String>) -> Failure {
    Failure::new(ErrorCode::InvalidInput, message, false)
}

fn invalid_state(message: impl Into<String>) -> Failure {
    Failure::new(ErrorCode::InvalidState, message, false)
}
