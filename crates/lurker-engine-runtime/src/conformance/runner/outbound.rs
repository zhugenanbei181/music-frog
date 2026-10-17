//! Outbound inventory, selection, observation, details and probe checks.

use super::super::case::{AdapterConformanceCase, DetailsBehavior, ProbeBehavior};
use super::super::failure::{ConformanceCheck, ConformanceFailure};
use lurker_core_contract::identity::CoreScope;
use lurker_core_contract::outbound::details::OutboundDetailsState;
use lurker_core_contract::outbound::identity::{OutboundId, SnapshotRevision};
use lurker_core_contract::outbound::inventory::{
    OutboundInventorySnapshot, OutboundInventoryState, OutboundProbeState, OutboundSelectionState,
};
use lurker_core_ports::engine_backend::EngineBackendPorts;
use lurker_core_ports::outbound::OutboundPort;

pub(super) async fn verify_outbound(
    case: &AdapterConformanceCase,
    backend: &EngineBackendPorts,
    scope: &CoreScope,
) -> Result<(), ConformanceFailure> {
    let outbound = backend.operations().outbound().map_err(|error| {
        ConformanceFailure::port_error(
            ConformanceCheck::OutboundScope,
            "outbound operations are composed",
            "no outbound adapter is available",
            &error,
        )
    })?;
    let inventory = outbound.inventory(scope.clone()).await.map_err(|error| {
        ConformanceFailure::port_error(
            ConformanceCheck::OutboundInventory,
            "a ready inventory",
            "the outbound inventory read failed",
            &error,
        )
    })?;
    let initial = verify_inventory(case, &inventory, scope)?;
    if initial != case.initial_member_id {
        return Err(ConformanceFailure::mismatch(
            ConformanceCheck::OutboundSelection,
            case.initial_member_id.value(),
            initial.value(),
            "the selector did not start on the initial member",
        ));
    }

    verify_details(case, outbound.as_ref(), scope, inventory.revision()).await?;

    let inventory = outbound.inventory(scope.clone()).await.map_err(|error| {
        ConformanceFailure::port_error(
            ConformanceCheck::OutboundInventory,
            "a ready inventory",
            "the inventory read before the probe failed",
            &error,
        )
    })?;
    let after_probe = verify_probe(case, outbound.as_ref(), scope, inventory.revision()).await?;

    let selected = outbound
        .select(
            scope.clone(),
            after_probe.revision(),
            case.collection_id.clone(),
            case.alternate_member_id.clone(),
        )
        .await
        .map_err(|error| {
            ConformanceFailure::port_error(
                ConformanceCheck::OutboundSelection,
                "the alternate member is selectable",
                "the selector write or read-back failed",
                &error,
            )
        })?;
    let selected_member = selection_from_inventory(&selected, &case.collection_id)?;
    if selected_member != case.alternate_member_id {
        return Err(ConformanceFailure::mismatch(
            ConformanceCheck::OutboundSelection,
            case.alternate_member_id.value(),
            selected_member.value(),
            "the selector write did not read back the requested member",
        ));
    }

    let observation = outbound.observation(scope.clone()).await.map_err(|error| {
        ConformanceFailure::port_error(
            ConformanceCheck::OutboundObservation,
            "one inventory/details observation",
            "the composed observation failed",
            &error,
        )
    })?;
    if observation.inventory().scope() != scope {
        return Err(ConformanceFailure::state(
            ConformanceCheck::OutboundObservation,
            "the active CoreScope",
            format!("{:?}", observation.inventory().scope()),
            "the composed observation escaped the active scope",
        ));
    }
    let observed_member = selection_from_inventory(observation.inventory(), &case.collection_id)?;
    if observed_member != case.alternate_member_id {
        return Err(ConformanceFailure::mismatch(
            ConformanceCheck::OutboundObservation,
            case.alternate_member_id.value(),
            observed_member.value(),
            "the observation did not keep the selected member",
        ));
    }
    verify_details_state(
        case,
        observation.details().state(),
        ConformanceCheck::OutboundObservation,
    )?;

    let restore_revision = observation.inventory().revision();
    let restored = outbound
        .select(
            scope.clone(),
            restore_revision,
            case.collection_id.clone(),
            case.initial_member_id.clone(),
        )
        .await
        .map_err(|error| {
            ConformanceFailure::port_error(
                ConformanceCheck::OutboundRestore,
                "the initial member is restored",
                "the selector restore or read-back failed",
                &error,
            )
        })?;
    let restored_member = selection_from_inventory(&restored, &case.collection_id)?;
    if restored_member != case.initial_member_id {
        return Err(ConformanceFailure::mismatch(
            ConformanceCheck::OutboundRestore,
            case.initial_member_id.value(),
            restored_member.value(),
            "the selector restore did not read back the initial member",
        ));
    }
    Ok(())
}

fn verify_inventory(
    case: &AdapterConformanceCase,
    snapshot: &OutboundInventorySnapshot,
    scope: &CoreScope,
) -> Result<OutboundId, ConformanceFailure> {
    if snapshot.scope() != scope {
        return Err(ConformanceFailure::state(
            ConformanceCheck::OutboundScope,
            "the active CoreScope",
            format!("{:?}", snapshot.scope()),
            "the outbound inventory escaped the active scope",
        ));
    }
    let OutboundInventoryState::Ready { collections, .. } = snapshot.inventory() else {
        return Err(ConformanceFailure::state(
            ConformanceCheck::OutboundInventory,
            "a ready inventory",
            format!("{:?}", snapshot.inventory()),
            "the outbound inventory is not ready",
        ));
    };
    let collection = collections
        .iter()
        .find(|collection| collection.id == case.collection_id)
        .ok_or_else(|| {
            ConformanceFailure::state(
                ConformanceCheck::OutboundInventory,
                format!("selector {}", case.collection_id.value()),
                "collection absent",
                "the configured selector is missing from the inventory",
            )
        })?;
    for member in [&case.initial_member_id, &case.alternate_member_id] {
        if !collection.member_ids.contains(member) {
            return Err(ConformanceFailure::state(
                ConformanceCheck::OutboundInventory,
                format!(
                    "member {} in selector {}",
                    member.value(),
                    case.collection_id.value()
                ),
                "member absent",
                "the configured selector member is missing from the inventory",
            ));
        }
    }
    match &collection.selection {
        OutboundSelectionState::Ready { selected } => Ok(selected.clone()),
        other => Err(ConformanceFailure::state(
            ConformanceCheck::OutboundSelection,
            "a ready selection",
            format!("{other:?}"),
            "the selector did not expose a current member",
        )),
    }
}

pub(super) fn selection_from_inventory(
    snapshot: &OutboundInventorySnapshot,
    collection_id: &OutboundId,
) -> Result<OutboundId, ConformanceFailure> {
    let OutboundInventoryState::Ready { collections, .. } = snapshot.inventory() else {
        return Err(ConformanceFailure::state(
            ConformanceCheck::OutboundInventory,
            "a ready inventory",
            format!("{:?}", snapshot.inventory()),
            "the outbound inventory is not ready",
        ));
    };
    let collection = collections
        .iter()
        .find(|collection| &collection.id == collection_id)
        .ok_or_else(|| {
            ConformanceFailure::state(
                ConformanceCheck::OutboundInventory,
                format!("selector {}", collection_id.value()),
                "collection absent",
                "the configured selector is missing from the inventory",
            )
        })?;
    match &collection.selection {
        OutboundSelectionState::Ready { selected } => Ok(selected.clone()),
        other => Err(ConformanceFailure::state(
            ConformanceCheck::OutboundSelection,
            "a ready selection",
            format!("{other:?}"),
            "the selector did not expose a current member",
        )),
    }
}

async fn verify_details(
    case: &AdapterConformanceCase,
    outbound: &dyn OutboundPort,
    scope: &CoreScope,
    revision: SnapshotRevision,
) -> Result<(), ConformanceFailure> {
    let details = outbound
        .details(scope.clone(), revision)
        .await
        .map_err(|error| {
            ConformanceFailure::port_error(
                ConformanceCheck::OutboundDetails,
                "a neutral details state",
                "the details read failed",
                &error,
            )
        })?;
    if details.scope() != scope {
        return Err(ConformanceFailure::state(
            ConformanceCheck::OutboundDetails,
            "the active CoreScope",
            format!("{:?}", details.scope()),
            "the details escaped the active scope",
        ));
    }
    verify_details_state(case, details.state(), ConformanceCheck::OutboundDetails)
}

fn verify_details_state(
    case: &AdapterConformanceCase,
    state: &OutboundDetailsState,
    check: ConformanceCheck,
) -> Result<(), ConformanceFailure> {
    match (&case.details, state) {
        (DetailsBehavior::Ready, OutboundDetailsState::Ready { details })
            if details
                .iter()
                .any(|detail| detail.id == case.probe_outbound_id) =>
        {
            Ok(())
        }
        (DetailsBehavior::Unsupported, OutboundDetailsState::Unsupported) => Ok(()),
        (expected, actual) => Err(ConformanceFailure::mismatch(
            check,
            format!("{expected:?}"),
            format!("{actual:?}"),
            "the details state does not match the case",
        )),
    }
}

async fn verify_probe(
    case: &AdapterConformanceCase,
    outbound: &dyn OutboundPort,
    scope: &CoreScope,
    revision: SnapshotRevision,
) -> Result<OutboundInventorySnapshot, ConformanceFailure> {
    let probe = outbound
        .probe(
            scope.clone(),
            revision,
            case.probe_outbound_id.clone(),
            case.probe_request.clone(),
        )
        .await
        .map_err(|error| {
            ConformanceFailure::port_error(
                ConformanceCheck::OutboundProbe,
                "a neutral probe result",
                "the outbound probe failed",
                &error,
            )
        })?;
    if probe.scope() != scope || probe.outbound_id() != &case.probe_outbound_id {
        return Err(ConformanceFailure::state(
            ConformanceCheck::OutboundProbe,
            "a result fenced to the active scope and probe outbound",
            format!("{:?} / {}", probe.scope(), probe.outbound_id().value()),
            "the probe result escaped its request",
        ));
    }
    match (&case.probe, probe.state()) {
        (ProbeBehavior::Measured, OutboundProbeState::Measured { .. }) => {}
        (ProbeBehavior::Unsupported, OutboundProbeState::Unsupported) => {}
        (expected, actual) => {
            return Err(ConformanceFailure::mismatch(
                ConformanceCheck::OutboundProbe,
                format!("{expected:?}"),
                format!("{actual:?}"),
                "the probe state does not match the case",
            ));
        }
    }

    let after_probe = outbound.inventory(scope.clone()).await.map_err(|error| {
        ConformanceFailure::port_error(
            ConformanceCheck::OutboundProbe,
            "a stable inventory after the probe",
            "the inventory read after the probe failed",
            &error,
        )
    })?;
    if after_probe.scope() != scope {
        return Err(ConformanceFailure::state(
            ConformanceCheck::OutboundProbe,
            "the active CoreScope",
            format!("{:?}", after_probe.scope()),
            "the outbound probe changed the observed scope",
        ));
    }
    let member = selection_from_inventory(&after_probe, &case.collection_id)?;
    if member != case.initial_member_id {
        return Err(ConformanceFailure::state(
            ConformanceCheck::OutboundProbe,
            case.initial_member_id.value(),
            member.value(),
            "the outbound probe changed the current selector member",
        ));
    }
    if case.probe == ProbeBehavior::Unsupported && after_probe.revision() != revision {
        return Err(ConformanceFailure::state(
            ConformanceCheck::OutboundProbe,
            format!("revision {}", revision.value()),
            format!("revision {}", after_probe.revision().value()),
            "an unsupported probe must not change the inventory revision",
        ));
    }
    Ok(after_probe)
}
