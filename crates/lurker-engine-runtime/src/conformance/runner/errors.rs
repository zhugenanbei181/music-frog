//! Routing read-back and typed-error conformance checks.

use super::super::case::{AdapterConformanceCase, RoutingBehavior};
use super::super::failure::{ConformanceCheck, ConformanceFailure};
use super::outbound::selection_from_inventory;
use lurker_core_contract::error::ErrorCode;
use lurker_core_contract::identity::{CoreGeneration, CoreScope, CoreVersion, EngineId};
use lurker_core_contract::outbound::identity::SnapshotRevision;
use lurker_core_contract::session::SessionToken;
use lurker_core_ports::engine_backend::EngineBackendPorts;

pub(super) async fn verify_routing(
    case: &AdapterConformanceCase,
    backend: &EngineBackendPorts,
    scope: &CoreScope,
) -> Result<(), ConformanceFailure> {
    let routing = backend.operations().routing_intent().map_err(|error| {
        ConformanceFailure::port_error(
            ConformanceCheck::RoutingObservation,
            "routing-intent operations are composed",
            "no routing adapter is available",
            &error,
        )
    })?;
    match &case.routing {
        RoutingBehavior::Writable { initial, requested } => {
            let observed = routing.observe(scope).await.map_err(|error| {
                ConformanceFailure::port_error(
                    ConformanceCheck::RoutingObservation,
                    format!("{initial:?}"),
                    "the initial routing observation failed",
                    &error,
                )
            })?;
            if &observed != initial {
                return Err(ConformanceFailure::mismatch(
                    ConformanceCheck::RoutingObservation,
                    format!("{initial:?}"),
                    format!("{observed:?}"),
                    "the initial routing intent differs from the case",
                ));
            }
            let changed = routing
                .apply(scope, requested.clone())
                .await
                .map_err(|error| {
                    ConformanceFailure::port_error(
                        ConformanceCheck::RoutingReadback,
                        format!("{requested:?}"),
                        "the routing write failed",
                        &error,
                    )
                })?;
            if &changed != requested {
                return Err(ConformanceFailure::mismatch(
                    ConformanceCheck::RoutingReadback,
                    format!("{requested:?}"),
                    format!("{changed:?}"),
                    "the routing write returned a mismatching read-back",
                ));
            }
            let restored = routing
                .apply(scope, initial.clone())
                .await
                .map_err(|error| {
                    ConformanceFailure::port_error(
                        ConformanceCheck::RoutingReadback,
                        format!("{initial:?}"),
                        "the routing restore failed",
                        &error,
                    )
                })?;
            if &restored != initial {
                return Err(ConformanceFailure::mismatch(
                    ConformanceCheck::RoutingReadback,
                    format!("{initial:?}"),
                    format!("{restored:?}"),
                    "the routing restore returned a mismatching read-back",
                ));
            }
            Ok(())
        }
        RoutingBehavior::ReadOnlyUnsupported { initial, requested } => {
            let observed = routing.observe(scope).await.map_err(|error| {
                ConformanceFailure::port_error(
                    ConformanceCheck::RoutingObservation,
                    format!("{initial:?}"),
                    "the initial routing observation failed",
                    &error,
                )
            })?;
            if &observed != initial {
                return Err(ConformanceFailure::mismatch(
                    ConformanceCheck::RoutingObservation,
                    format!("{initial:?}"),
                    format!("{observed:?}"),
                    "the initial routing intent differs from the case",
                ));
            }
            match routing.apply(scope, requested.clone()).await {
                Err(error) if error.error_code() == ErrorCode::Unsupported => {}
                Err(error) => {
                    return Err(ConformanceFailure::adapter_error(
                        ConformanceCheck::RoutingUnsupported,
                        "a typed Unsupported error",
                        error.to_string(),
                        "an unsupported routing write returned another error",
                    ));
                }
                Ok(_) => {
                    return Err(ConformanceFailure::state(
                        ConformanceCheck::RoutingUnsupported,
                        "a typed Unsupported error",
                        "write accepted",
                        "an unsupported routing write was accepted",
                    ));
                }
            }
            let after = routing.observe(scope).await.map_err(|error| {
                ConformanceFailure::port_error(
                    ConformanceCheck::RoutingUnsupported,
                    format!("{initial:?}"),
                    "the routing read-back after rejection failed",
                    &error,
                )
            })?;
            if &after != initial {
                return Err(ConformanceFailure::state(
                    ConformanceCheck::RoutingUnsupported,
                    format!("{initial:?}"),
                    format!("{after:?}"),
                    "an unsupported routing write changed the observed intent",
                ));
            }
            Ok(())
        }
    }
}

pub(super) async fn verify_typed_errors(
    case: &AdapterConformanceCase,
    backend: &EngineBackendPorts,
    scope: &CoreScope,
) -> Result<(), ConformanceFailure> {
    let outbound = backend.operations().outbound().map_err(|error| {
        ConformanceFailure::port_error(
            ConformanceCheck::OutboundScope,
            "outbound operations are composed",
            "no outbound adapter is available for the typed-error group",
            &error,
        )
    })?;
    let inventory = outbound.inventory(scope.clone()).await.map_err(|error| {
        ConformanceFailure::port_error(
            ConformanceCheck::OutboundInventory,
            "a ready inventory",
            "the inventory read before the typed-error group failed",
            &error,
        )
    })?;
    let expected_revision = inventory.revision();
    let expected_member = selection_from_inventory(&inventory, &case.collection_id)?;
    if expected_member != case.initial_member_id {
        return Err(ConformanceFailure::mismatch(
            ConformanceCheck::OutboundSelection,
            case.initial_member_id.value(),
            expected_member.value(),
            "the typed-error group requires the initial selector member",
        ));
    }

    let next_generation = scope.generation.value().checked_add(1).ok_or_else(|| {
        ConformanceFailure::state(
            ConformanceCheck::ErrorStaleGeneration,
            "an incremented generation",
            "overflow",
            "the generation cannot advance for the stale-scope check",
        )
    })?;
    let mut stale_generation = scope.clone();
    stale_generation.generation = CoreGeneration::new(next_generation);
    let mut stale_version = scope.clone();
    stale_version.identity.version = Some(
        CoreVersion::new("conformance-stale-version").expect("the static stale version is valid"),
    );
    let mut stale_session = scope.clone();
    stale_session.session_token = Some(SessionToken::new(2));
    let mut foreign_engine = scope.clone();
    foreign_engine.identity.engine =
        EngineId::new("conformance-foreign-engine").expect("the static foreign engine is valid");

    for (label, check, rejected) in [
        (
            "stale generation",
            ConformanceCheck::ErrorStaleGeneration,
            stale_generation,
        ),
        (
            "stale version",
            ConformanceCheck::ErrorStaleVersion,
            stale_version,
        ),
        (
            "stale session",
            ConformanceCheck::ErrorStaleSession,
            stale_session,
        ),
        (
            "foreign engine",
            ConformanceCheck::ErrorForeignEngine,
            foreign_engine,
        ),
    ] {
        match outbound
            .select(
                rejected,
                expected_revision,
                case.collection_id.clone(),
                case.alternate_member_id.clone(),
            )
            .await
        {
            Err(error) if error.error_code() == ErrorCode::InvalidState => {}
            Err(error) => {
                return Err(ConformanceFailure::adapter_error(
                    check,
                    "a typed InvalidState error",
                    error.to_string(),
                    format!("the {label} scope returned another error"),
                ));
            }
            Ok(_) => {
                return Err(ConformanceFailure::state(
                    check,
                    "a typed InvalidState error",
                    "selection accepted",
                    format!("the {label} scope was accepted"),
                ));
            }
        }
        let after = outbound.inventory(scope.clone()).await.map_err(|error| {
            ConformanceFailure::port_error(
                check,
                "a readable inventory after rejection",
                format!("the inventory read after the {label} rejection failed"),
                &error,
            )
        })?;
        if after.revision() != expected_revision {
            return Err(ConformanceFailure::state(
                check,
                format!("revision {}", expected_revision.value()),
                format!("revision {}", after.revision().value()),
                format!("the {label} rejection changed the inventory revision"),
            ));
        }
        let member = selection_from_inventory(&after, &case.collection_id)?;
        if member != case.initial_member_id {
            return Err(ConformanceFailure::state(
                check,
                case.initial_member_id.value(),
                member.value(),
                format!("the {label} rejection changed the selector member"),
            ));
        }
    }

    let next_revision = expected_revision.value().checked_add(1).ok_or_else(|| {
        ConformanceFailure::state(
            ConformanceCheck::ErrorStaleRevision,
            "an incremented inventory revision",
            "overflow",
            "the inventory revision cannot advance for the stale-write check",
        )
    })?;
    match outbound
        .select(
            scope.clone(),
            SnapshotRevision::new(next_revision),
            case.collection_id.clone(),
            case.alternate_member_id.clone(),
        )
        .await
    {
        Err(error) if error.error_code() == ErrorCode::InvalidState => {}
        Err(error) => {
            return Err(ConformanceFailure::adapter_error(
                ConformanceCheck::ErrorStaleRevision,
                "a typed InvalidState error",
                error.to_string(),
                "the stale inventory revision returned another error",
            ));
        }
        Ok(_) => {
            return Err(ConformanceFailure::state(
                ConformanceCheck::ErrorStaleRevision,
                "a typed InvalidState error",
                "selection accepted",
                "the stale inventory revision was accepted",
            ));
        }
    }
    let after = outbound.inventory(scope.clone()).await.map_err(|error| {
        ConformanceFailure::port_error(
            ConformanceCheck::ErrorStaleRevision,
            "a readable inventory after rejection",
            "the inventory read after the stale-revision rejection failed",
            &error,
        )
    })?;
    if after.revision() != expected_revision {
        return Err(ConformanceFailure::state(
            ConformanceCheck::ErrorStaleRevision,
            format!("revision {}", expected_revision.value()),
            format!("revision {}", after.revision().value()),
            "the stale revision rejection changed the inventory revision",
        ));
    }
    let member = selection_from_inventory(&after, &case.collection_id)?;
    if member != case.initial_member_id {
        return Err(ConformanceFailure::state(
            ConformanceCheck::ErrorStaleRevision,
            case.initial_member_id.value(),
            member.value(),
            "the stale revision rejection changed the selector member",
        ));
    }
    Ok(())
}
