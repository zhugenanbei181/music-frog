//! Neutral owner-switch conformance through the coordinator.
//!
//! The kit binds the first engine when the journal is unowned, starts it, then
//! runs a full coordinator switch with an exact host trace. It then proves the
//! pre-reservation rollback path and the post-old-terminal `RecoveryRequired`
//! path. The final journal state is unowned and every process is stopped.

mod cleanup;
mod host;

use self::cleanup::{cleanup_processes, combine_switch_result};
use self::host::{ConformanceSwitchHost, SwitchHostEvent};
use super::failure::{ConformanceCheck, ConformanceFailure};
use super::readiness::wait_for_readiness;
use crate::registry::EngineBackendProviderRegistry;
use crate::switch::coordinator::EngineSwitchCoordinator;
use crate::switch::error::EngineSwitchError;
use crate::switch::host::EngineSwitchHost;
use lurker_core_contract::core_process::CoreProcessStartContext;
use lurker_core_contract::engine_switch::{
    EngineOwnerState, EngineSwitchDisposition, EngineSwitchFailurePhase, EngineSwitchRecovery,
    EngineSwitchStage, EngineSwitchTarget,
};
use lurker_core_contract::identity::{CoreGeneration, CoreIdentity, CoreInstanceId, EngineId};
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_ports::core_process::{CoreProcess, CoreReadiness};
use lurker_core_ports::engine_backend::EngineBackendPorts;
use lurker_core_ports::engine_switch::EngineSwitchJournal;
use lurker_core_ports::error::PortError;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Default bound for a candidate readiness wait during the switch kit.
pub const DEFAULT_SWITCH_READINESS_TIMEOUT: Duration = Duration::from_secs(10);

/// Prove the complete switch protocol for two registered engines.
///
/// The journal may be fresh (unowned, selection = `first`) or already bound to
/// `first`. On success every process is stopped and the journal is unowned.
pub async fn run_switch_conformance<J>(
    registry: Arc<EngineBackendProviderRegistry<EngineBackendPorts>>,
    journal: Arc<J>,
    first: &EngineId,
    second: &EngineId,
) -> Result<(), ConformanceFailure>
where
    J: EngineSwitchJournal<Selection = EngineId> + 'static + ?Sized,
{
    if first == second {
        return Err(ConformanceFailure::state(
            ConformanceCheck::SwitchTarget,
            "two distinct engines",
            first.as_str(),
            "the switch kit requires a real owner replacement",
        ));
    }
    let first_backend = registry.backend(first).map_err(|error| {
        journal_failure(
            ConformanceCheck::SwitchTarget,
            "the first engine has a backend provider",
            &error,
        )
    })?;
    let second_backend = registry.backend(second).map_err(|error| {
        journal_failure(
            ConformanceCheck::SwitchTarget,
            "the second engine has a backend provider",
            &error,
        )
    })?;
    let first_process = first_backend.process();
    let first_readiness = first_backend.readiness();
    let second_process = second_backend.process();

    let result = run_switch_flow(
        Arc::clone(&registry),
        Arc::clone(&journal),
        first,
        second,
        Arc::clone(&first_process),
        first_readiness,
        Arc::clone(&second_process),
    )
    .await;
    let cleanup = cleanup_processes(&[first_process, second_process]).await;
    combine_switch_result(result, cleanup)
}

async fn run_switch_flow<J>(
    registry: Arc<EngineBackendProviderRegistry<EngineBackendPorts>>,
    journal: Arc<J>,
    first: &EngineId,
    second: &EngineId,
    first_process: Arc<dyn CoreProcess>,
    first_readiness: Arc<dyn CoreReadiness>,
    second_process: Arc<dyn CoreProcess>,
) -> Result<(), ConformanceFailure>
where
    J: EngineSwitchJournal<Selection = EngineId> + 'static + ?Sized,
{
    let first_identity = process_identity(&first_process, first)?;
    let second_identity = process_identity(&second_process, second)?;

    let snapshot = journal.snapshot().await.map_err(|error| {
        journal_failure(
            ConformanceCheck::SwitchJournal,
            "the journal is readable",
            &error,
        )
    })?;
    let old_generation = match &snapshot.owner {
        EngineOwnerState::Unowned => {
            if snapshot.selection != *first {
                return Err(ConformanceFailure::state(
                    ConformanceCheck::SwitchJournal,
                    format!("unowned selection {}", first.as_str()),
                    snapshot.selection.as_str(),
                    "an unowned journal must already select the first engine",
                ));
            }
            ensure_running(&first_process, &first_readiness, &first_identity).await?;
            let generation = CoreGeneration::new(1);
            let (bound, initial_lease) = journal
                .bind_owner(
                    snapshot.revision,
                    first.clone(),
                    first_identity.clone(),
                    generation,
                )
                .await
                .map_err(|error| {
                    journal_failure(
                        ConformanceCheck::SwitchJournal,
                        "the first owner binds",
                        &error,
                    )
                })?;
            match &bound.owner {
                EngineOwnerState::Bound {
                    identity,
                    generation: bound_generation,
                } if identity.same_instance(&first_identity) && *bound_generation == generation => {
                }
                other => {
                    return Err(ConformanceFailure::state(
                        ConformanceCheck::SwitchJournal,
                        "the exact first owner binding",
                        format!("{other:?}"),
                        "bind_owner returned another owner state",
                    ));
                }
            }
            // Dropping the proof never clears the journal; the coordinator
            // replaces the binding through its own revision-fenced leases.
            drop(initial_lease);
            generation
        }
        EngineOwnerState::Bound {
            identity,
            generation,
        } => {
            if snapshot.selection != *first || identity.engine != *first {
                return Err(ConformanceFailure::state(
                    ConformanceCheck::SwitchJournal,
                    format!("the first owner {}", first.as_str()),
                    format!(
                        "selection {} / identity {}",
                        snapshot.selection.as_str(),
                        identity.engine.as_str()
                    ),
                    "the bound journal owner does not match the first engine",
                ));
            }
            let live = process_identity(&first_process, first)?;
            if !live.same_instance(identity) {
                return Err(ConformanceFailure::state(
                    ConformanceCheck::SwitchJournal,
                    format!("instance {}", identity.instance.as_str()),
                    format!("instance {}", live.instance.as_str()),
                    "the process adapter is not the journal-bound owner",
                ));
            }
            ensure_running(&first_process, &first_readiness, identity).await?;
            *generation
        }
        EngineOwnerState::Switching { stage } => {
            return Err(ConformanceFailure::state(
                ConformanceCheck::SwitchJournal,
                "a settled journal",
                format!("{stage:?}"),
                "the switch kit requires a settled journal",
            ));
        }
    };

    let before = journal.snapshot().await.map_err(|error| {
        journal_failure(
            ConformanceCheck::SwitchJournal,
            "the journal is readable",
            &error,
        )
    })?;
    let successful_trace = empty_trace();
    let host = ConformanceSwitchHost::new(
        first.clone(),
        Arc::clone(&first_process),
        first_identity.clone(),
        old_generation,
        DEFAULT_SWITCH_READINESS_TIMEOUT,
        Arc::clone(&successful_trace),
    );
    let host: Arc<dyn EngineSwitchHost<EngineBackendPorts, EngineId>> = Arc::new(host);
    let coordinator =
        EngineSwitchCoordinator::new(Arc::clone(&registry), Arc::clone(&journal), host);
    let after = match coordinator
        .switch_and_finalize(before.revision, selected(second))
        .await
    {
        Ok(snapshot) => snapshot,
        Err(error) => {
            return Err(switch_failure(
                ConformanceCheck::SwitchCoordinator,
                "the coordinator completes the forward switch",
                error,
            ));
        }
    };
    expect_trace(
        &successful_trace,
        &[
            SwitchHostEvent::ValidateTarget,
            SwitchHostEvent::PrepareCandidate,
            SwitchHostEvent::OldOwnerStopRequested,
            SwitchHostEvent::OldOwnerTerminalObserved,
            SwitchHostEvent::NewOwnerStartRequested,
            SwitchHostEvent::NewOwnerReady,
        ],
    )?;
    assert_old_terminal_before_new_start(&successful_trace)?;
    expect_status(
        &first_process,
        CoreLifecycle::Stopped,
        ConformanceCheck::SwitchOrdering,
        "the old owner reaches its exact terminal",
    )
    .await?;
    expect_status(
        &second_process,
        CoreLifecycle::Running,
        ConformanceCheck::SwitchOrdering,
        "the new owner is running after the old terminal",
    )
    .await?;
    if after.selection != *second {
        return Err(ConformanceFailure::mismatch(
            ConformanceCheck::SwitchJournal,
            second.as_str(),
            after.selection.as_str(),
            "the journal selection did not change to the second engine",
        ));
    }
    match &after.owner {
        EngineOwnerState::Bound {
            identity,
            generation,
        } if identity.same_instance(&second_identity) && *generation == CoreGeneration::new(1) => {}
        other => {
            return Err(ConformanceFailure::state(
                ConformanceCheck::SwitchJournal,
                "the exact second owner binding",
                format!("{other:?}"),
                "the coordinator did not bind the applied candidate",
            ));
        }
    }

    let before_rollback = journal.snapshot().await.map_err(|error| {
        journal_failure(
            ConformanceCheck::SwitchJournal,
            "the journal is readable",
            &error,
        )
    })?;
    let rollback_trace = empty_trace();
    let host = ConformanceSwitchHost::new(
        second.clone(),
        Arc::clone(&second_process),
        second_identity.clone(),
        CoreGeneration::new(1),
        DEFAULT_SWITCH_READINESS_TIMEOUT,
        Arc::clone(&rollback_trace),
    );
    let host: Arc<dyn EngineSwitchHost<EngineBackendPorts, EngineId>> = Arc::new(host);
    let coordinator =
        EngineSwitchCoordinator::new(Arc::clone(&registry), Arc::clone(&journal), host);
    let mismatch = EngineSwitchTarget::Selected {
        engine_id: second.clone(),
        selection: first.clone(),
    };
    let rollback_failure = match coordinator
        .switch_and_finalize(before_rollback.revision, mismatch)
        .await
    {
        Ok(_) => {
            return Err(ConformanceFailure::state(
                ConformanceCheck::SwitchTarget,
                "a rejected mismatching target",
                "switch accepted",
                "the coordinator accepted a target whose engine and selection differ",
            ));
        }
        Err(error) => error,
    };
    if rollback_failure.phase != EngineSwitchFailurePhase::BeforeReservation {
        return Err(switch_failure(
            ConformanceCheck::SwitchTarget,
            "a pre-reservation rollback",
            rollback_failure,
        ));
    }
    match rollback_failure.disposition {
        EngineSwitchDisposition::RolledBackToPrevious { selection } if selection == *second => {}
        other => {
            return Err(ConformanceFailure::state(
                ConformanceCheck::SwitchTarget,
                format!("RolledBackToPrevious({})", second.as_str()),
                format!("{other:?}"),
                "a rejected target must not require recovery",
            ));
        }
    }
    let after_rollback = journal.snapshot().await.map_err(|error| {
        journal_failure(
            ConformanceCheck::SwitchJournal,
            "the journal is readable",
            &error,
        )
    })?;
    if after_rollback != before_rollback {
        return Err(ConformanceFailure::state(
            ConformanceCheck::SwitchJournal,
            "an unchanged journal after rollback",
            format!("{after_rollback:?}"),
            "the pre-reservation rollback changed the journal",
        ));
    }
    expect_trace(&rollback_trace, &[SwitchHostEvent::ValidateTarget])?;
    expect_status(
        &second_process,
        CoreLifecycle::Running,
        ConformanceCheck::SwitchOrdering,
        "the previous owner stays running after rollback",
    )
    .await?;

    let unregistered = EngineId::new("conformance-unregistered")
        .expect("the static unregistered engine ID is valid");
    let before_unregistered = journal.snapshot().await.map_err(|error| {
        journal_failure(
            ConformanceCheck::SwitchJournal,
            "the journal is readable",
            &error,
        )
    })?;
    let unregistered_trace = empty_trace();
    let host = ConformanceSwitchHost::new(
        second.clone(),
        Arc::clone(&second_process),
        second_identity.clone(),
        CoreGeneration::new(1),
        DEFAULT_SWITCH_READINESS_TIMEOUT,
        Arc::clone(&unregistered_trace),
    );
    let host: Arc<dyn EngineSwitchHost<EngineBackendPorts, EngineId>> = Arc::new(host);
    let coordinator =
        EngineSwitchCoordinator::new(Arc::clone(&registry), Arc::clone(&journal), host);
    let unregistered_failure = match coordinator
        .switch_and_finalize(before_unregistered.revision, selected(&unregistered))
        .await
    {
        Ok(_) => {
            return Err(ConformanceFailure::state(
                ConformanceCheck::SwitchTarget,
                "an unregistered target is rejected",
                "switch accepted",
                "the coordinator accepted an engine without a provider",
            ));
        }
        Err(error) => error,
    };
    if unregistered_failure.phase != EngineSwitchFailurePhase::BeforeReservation
        || !matches!(
            unregistered_failure.disposition,
            EngineSwitchDisposition::RolledBackToPrevious { .. }
        )
    {
        return Err(switch_failure(
            ConformanceCheck::SwitchTarget,
            "an unregistered target rolls back before reservation",
            unregistered_failure,
        ));
    }
    let after_unregistered = journal.snapshot().await.map_err(|error| {
        journal_failure(
            ConformanceCheck::SwitchJournal,
            "the journal is readable",
            &error,
        )
    })?;
    if after_unregistered != before_unregistered {
        return Err(ConformanceFailure::state(
            ConformanceCheck::SwitchJournal,
            "an unchanged journal after the unregistered target",
            format!("{after_unregistered:?}"),
            "an unregistered target changed the journal",
        ));
    }
    expect_trace(&unregistered_trace, &[SwitchHostEvent::ValidateTarget])?;

    let before_recovery = journal.snapshot().await.map_err(|error| {
        journal_failure(
            ConformanceCheck::SwitchJournal,
            "the journal is readable",
            &error,
        )
    })?;
    let recovery_trace = empty_trace();
    let host = ConformanceSwitchHost::new(
        second.clone(),
        Arc::clone(&second_process),
        second_identity.clone(),
        CoreGeneration::new(1),
        DEFAULT_SWITCH_READINESS_TIMEOUT,
        Arc::clone(&recovery_trace),
    );
    let host: Arc<dyn EngineSwitchHost<EngineBackendPorts, EngineId>> = Arc::new(host);
    let coordinator =
        EngineSwitchCoordinator::new(Arc::clone(&registry), Arc::clone(&journal), host);
    let reservation = match coordinator
        .reserve_switch(before_recovery.revision, selected(first))
        .await
    {
        Ok(reservation) => reservation,
        Err(error) => {
            return Err(switch_failure(
                ConformanceCheck::SwitchRecovery,
                "the recovery switch reserves the first engine",
                error,
            ));
        }
    };
    expect_trace(
        &recovery_trace,
        &[
            SwitchHostEvent::ValidateTarget,
            SwitchHostEvent::PrepareCandidate,
            SwitchHostEvent::OldOwnerStopRequested,
            SwitchHostEvent::OldOwnerTerminalObserved,
        ],
    )?;
    expect_status(
        &second_process,
        CoreLifecycle::Stopped,
        ConformanceCheck::SwitchRecovery,
        "the old owner is terminal before the candidate admission",
    )
    .await?;
    expect_status(
        &first_process,
        CoreLifecycle::Stopped,
        ConformanceCheck::SwitchRecovery,
        "the candidate is not started before finalize",
    )
    .await?;
    let pending = journal.snapshot().await.map_err(|error| {
        journal_failure(
            ConformanceCheck::SwitchJournal,
            "the journal is readable",
            &error,
        )
    })?;
    match &pending.owner {
        EngineOwnerState::Switching {
            stage:
                EngineSwitchStage::BindingNew {
                    selection,
                    engine_id,
                },
        } if selection == first && engine_id == first => {}
        other => {
            return Err(ConformanceFailure::state(
                ConformanceCheck::SwitchRecovery,
                format!("BindingNew({})", first.as_str()),
                format!("{other:?}"),
                "the reservation did not reach the new-owner admission",
            ));
        }
    }

    let mismatched_identity = CoreIdentity::new(
        first.clone(),
        CoreInstanceId::new("conformance-mismatched-candidate")
            .expect("the static mismatched instance ID is valid"),
        None,
    );
    let recovery_failure = match coordinator
        .finalize_reserved(reservation, mismatched_identity, CoreGeneration::new(1))
        .await
    {
        Ok(_) => {
            return Err(ConformanceFailure::state(
                ConformanceCheck::SwitchRecovery,
                "RecoveryRequired for a mismatching candidate",
                "candidate bound",
                "the coordinator bound an identity that was never reserved",
            ));
        }
        Err(error) => error,
    };
    if !recovery_failure.is_recovery_required()
        || recovery_failure.phase != EngineSwitchFailurePhase::OldOwnerClosed
    {
        return Err(switch_failure(
            ConformanceCheck::SwitchRecovery,
            "RecoveryRequired after the old owner closed",
            recovery_failure,
        ));
    }
    let after_failure = journal.snapshot().await.map_err(|error| {
        journal_failure(
            ConformanceCheck::SwitchJournal,
            "the journal is readable",
            &error,
        )
    })?;
    if after_failure != pending {
        return Err(ConformanceFailure::state(
            ConformanceCheck::SwitchJournal,
            "the pending BindingNew stage",
            format!("{after_failure:?}"),
            "a failed finalize changed the reserved journal stage",
        ));
    }
    let recovery = journal
        .recover_owner_switch(pending.revision)
        .await
        .map_err(|error| {
            journal_failure(
                ConformanceCheck::SwitchRecovery,
                "the pending switch is observable",
                &error,
            )
        })?;
    match recovery {
        EngineSwitchRecovery::ResumeBindingNew {
            selection,
            engine_id,
        } if selection == *first && engine_id == *first => {}
        other => {
            return Err(ConformanceFailure::state(
                ConformanceCheck::SwitchRecovery,
                format!("ResumeBindingNew({})", first.as_str()),
                format!("{other:?}"),
                "the journal did not expose the resumable binding stage",
            ));
        }
    }
    let recovered = journal
        .recover_candidate_after_terminal(pending.revision, CoreGeneration::new(0))
        .await
        .map_err(|error| {
            journal_failure(
                ConformanceCheck::SwitchRecovery,
                "the reserved candidate recovers after its never-started terminal",
                &error,
            )
        })?;
    if recovered.owner != EngineOwnerState::Unowned || recovered.selection != *first {
        return Err(ConformanceFailure::state(
            ConformanceCheck::SwitchRecovery,
            "an unowned journal selecting the first engine",
            format!("{recovered:?}"),
            "candidate recovery did not clear the owner",
        ));
    }
    expect_status(
        &first_process,
        CoreLifecycle::Stopped,
        ConformanceCheck::SwitchRecovery,
        "the recovered candidate remains stopped",
    )
    .await?;
    expect_status(
        &second_process,
        CoreLifecycle::Stopped,
        ConformanceCheck::SwitchRecovery,
        "the terminated old owner remains stopped",
    )
    .await?;
    Ok(())
}

fn selected(engine: &EngineId) -> EngineSwitchTarget<EngineId> {
    EngineSwitchTarget::Selected {
        engine_id: engine.clone(),
        selection: engine.clone(),
    }
}

fn process_identity(
    process: &Arc<dyn CoreProcess>,
    engine: &EngineId,
) -> Result<CoreIdentity, ConformanceFailure> {
    let identity = process.core_identity().ok_or_else(|| {
        ConformanceFailure::state(
            ConformanceCheck::SwitchTarget,
            "a stable process identity",
            "None",
            format!("{} process has no CoreIdentity", engine.as_str()),
        )
    })?;
    if identity.engine != *engine {
        return Err(ConformanceFailure::mismatch(
            ConformanceCheck::SwitchTarget,
            engine.as_str(),
            identity.engine.as_str(),
            "the process adapter belongs to another engine",
        ));
    }
    Ok(identity)
}

async fn ensure_running(
    process: &Arc<dyn CoreProcess>,
    readiness: &Arc<dyn CoreReadiness>,
    identity: &CoreIdentity,
) -> Result<(), ConformanceFailure> {
    match process.status().await {
        Ok(CoreLifecycle::Running) => Ok(()),
        Ok(CoreLifecycle::Stopped) => {
            process
                .start(CoreProcessStartContext::standalone(identity.clone()))
                .await
                .map_err(|error| {
                    journal_failure(
                        ConformanceCheck::SwitchTarget,
                        "the first owner starts",
                        &error,
                    )
                })?;
            let version = wait_for_readiness(readiness.as_ref(), DEFAULT_SWITCH_READINESS_TIMEOUT)
                .await
                .map_err(|error| {
                    journal_failure(
                        ConformanceCheck::SwitchTarget,
                        "the first owner becomes ready",
                        &error,
                    )
                })?;
            if version.is_none() {
                return Err(ConformanceFailure::state(
                    ConformanceCheck::SwitchTarget,
                    "an active engine version",
                    "None",
                    "the started owner did not report its version",
                ));
            }
            match process.status().await {
                Ok(CoreLifecycle::Running) => Ok(()),
                Ok(other) => Err(ConformanceFailure::state(
                    ConformanceCheck::SwitchTarget,
                    "Running",
                    format!("{other:?}"),
                    "a ready owner is not backed by a running process",
                )),
                Err(error) => Err(journal_failure(
                    ConformanceCheck::SwitchTarget,
                    "the first owner state is readable",
                    &error,
                )),
            }
        }
        Ok(other) => Err(ConformanceFailure::state(
            ConformanceCheck::SwitchTarget,
            "Stopped or Running",
            format!("{other:?}"),
            "the journal-bound owner is in an unsupported lifecycle state",
        )),
        Err(error) => Err(journal_failure(
            ConformanceCheck::SwitchTarget,
            "the first owner state is readable",
            &error,
        )),
    }
}

async fn expect_status(
    process: &Arc<dyn CoreProcess>,
    expected: CoreLifecycle,
    check: ConformanceCheck,
    message: &str,
) -> Result<(), ConformanceFailure> {
    match process.status().await {
        Ok(actual) if actual == expected => Ok(()),
        Ok(actual) => Err(ConformanceFailure::state(
            check,
            format!("{expected:?}"),
            format!("{actual:?}"),
            message,
        )),
        Err(error) => Err(journal_failure(check, format!("{expected:?}"), &error)),
    }
}

fn empty_trace() -> Arc<Mutex<Vec<SwitchHostEvent>>> {
    Arc::new(Mutex::new(Vec::new()))
}

fn expect_trace(
    trace: &Arc<Mutex<Vec<SwitchHostEvent>>>,
    expected: &[SwitchHostEvent],
) -> Result<(), ConformanceFailure> {
    let actual = trace.lock().expect("switch conformance trace lock").clone();
    if actual == expected {
        Ok(())
    } else {
        Err(ConformanceFailure::state(
            ConformanceCheck::SwitchOrdering,
            format!("{expected:?}"),
            format!("{actual:?}"),
            "the switch host call order differs from the protocol",
        ))
    }
}

fn assert_old_terminal_before_new_start(
    trace: &Arc<Mutex<Vec<SwitchHostEvent>>>,
) -> Result<(), ConformanceFailure> {
    let actual = trace.lock().expect("switch conformance trace lock").clone();
    let terminal = actual
        .iter()
        .position(|event| *event == SwitchHostEvent::OldOwnerTerminalObserved);
    let start = actual
        .iter()
        .position(|event| *event == SwitchHostEvent::NewOwnerStartRequested);
    match (terminal, start) {
        (Some(terminal), Some(start)) if terminal < start => Ok(()),
        _ => Err(ConformanceFailure::state(
            ConformanceCheck::SwitchOrdering,
            "OldOwnerTerminalObserved before NewOwnerStartRequested",
            format!("{actual:?}"),
            "the candidate start was not strictly after the old owner terminal",
        )),
    }
}

fn switch_failure(
    check: ConformanceCheck,
    expected: &str,
    error: EngineSwitchError<EngineId>,
) -> ConformanceFailure {
    ConformanceFailure::adapter_error(
        check,
        expected,
        format!("{:?} / {:?}", error.phase, error.disposition),
        error.cause.to_string(),
    )
}

fn journal_failure(
    check: ConformanceCheck,
    expected: impl Into<String>,
    error: &PortError,
) -> ConformanceFailure {
    ConformanceFailure::port_error(
        check,
        expected,
        "the neutral journal or process operation failed",
        error,
    )
}
