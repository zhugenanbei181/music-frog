//! Cross-process Core owner normalization and terminal recovery shared by all engine stores.

use crate::active_owner_registry::{
    LocalOwnerRegistrationState, local_owner_registration_matches, local_owner_registration_state,
    lock_local_owner_registry,
};
use crate::active_reference_cas::{
    ActiveProfileReferenceCasReady, ActiveProfileReferenceOwnerFence,
    ActiveProfileReferenceTargetEvidence,
};
use crate::active_reference_file::ActiveProfileReferenceFile;
use crate::active_reference_owner::{
    ActiveCoreOwnerLaunchProof, ActiveCoreOwnerProof, ActiveCoreOwnerState,
    ActiveProfileReferenceRecord, LegacySelectionMigration, transition_rejected,
};
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReferenceRevision, ActiveProfileReferenceState, ActiveProfileReferenceTarget,
    ActiveProfileReferenceWriteAuthorization,
};
use infiltrator_contract::core_identity::{CoreGeneration, CoreIdentity, EngineId};
use infiltrator_contract::core_process::CoreProcessStartContext;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::snapshot::{CoreLifecycle, CoreLifecycleSnapshot};
use infiltrator_platform::instance_lock::ThreadedInstanceLockGuard;
use infiltrator_ports::active_profile_reference_store::{
    ActiveProfileOwnerRecoveryEvidence, ActiveProfileOwnerRecoveryOutcome,
};
use infiltrator_ports::error::PortError;
use std::future::Future;
use std::path::PathBuf;

#[derive(Clone, Copy)]
enum OwnerRecoveryRecordPosition {
    Bound,
    Unowned,
}

#[derive(Clone, Copy)]
enum OwnerRecoveryProofPosition {
    Bound,
    Terminal { terminal_generation: CoreGeneration },
}

enum OwnerRecoveryLaunchRequirement {
    NeverStarted,
    ManagedChildExited(CoreProcessStartContext),
}

/// Product-root owner-state reconciliation. The caller must hold the root
/// reference lock before normalizing; this coordinator acquires owner-state
/// after it, preserving the root → owner-state lock order.
pub struct ActiveProfileOwnerCoordinator {
    config_root: PathBuf,
}

impl ActiveProfileOwnerCoordinator {
    pub fn new(config_root: impl Into<PathBuf>) -> Self {
        Self {
            config_root: config_root.into(),
        }
    }

    pub(crate) fn config_root(&self) -> &std::path::Path {
        &self.config_root
    }

    /// Compare and publish a revisioned reference while the caller holds the
    /// product-root lock. The owner-state lock acquired here is retained until
    /// the durable reference publication completes.
    pub async fn compare_and_set_reference_unlocked<'a, F, Fut>(
        &'a self,
        record: &mut ActiveProfileReferenceRecord,
        expected_revision: ActiveProfileReferenceRevision,
        target: ActiveProfileReferenceTarget,
        authorization: ActiveProfileReferenceWriteAuthorization,
        local_engine: EngineId,
        load_target_evidence: F,
    ) -> Result<ActiveProfileReferenceState, PortError>
    where
        F: FnOnce(ActiveProfileReferenceTarget, EngineId) -> Fut + 'a,
        Fut: Future<Output = Result<ActiveProfileReferenceTargetEvidence, PortError>> + 'a,
    {
        let cas_ready =
            ActiveProfileReferenceCasReady::compare(&record.selection, expected_revision)?;
        let (owner_state, _owner_state_guard) = self.normalize_owner_state_unlocked(record).await?;
        validate_reference_write_authorization(&owner_state, &authorization, &self.config_root)?;
        let target_evidence = load_target_evidence(target.clone(), local_engine.clone()).await?;
        validate_target_evidence(&target, &local_engine, target_evidence)?;

        let owner_fence = match &owner_state {
            ActiveCoreOwnerState::Unowned => ActiveProfileReferenceOwnerFence::Unowned,
            ActiveCoreOwnerState::Bound {
                identity,
                generation,
                ..
            } => ActiveProfileReferenceOwnerFence::Bound {
                identity: identity.clone(),
                generation: *generation,
            },
            ActiveCoreOwnerState::Switching { .. } => ActiveProfileReferenceOwnerFence::Switching,
        };
        record.selection = cas_ready.transition(target, owner_fence)?;
        record.legacy_migration = LegacySelectionMigration::ExplicitSelection;
        ActiveProfileReferenceFile::new(self.config_root.clone())
            .publish_active_profile_record_sync_reconciled(record)?;
        Ok(record.selection.clone())
    }

    /// Reconcile a persisted owner only after the host has supplied exact
    /// process-exit evidence and a matching terminal lifecycle snapshot.
    pub async fn recover_bound_owner_after_terminal(
        &self,
        evidence: ActiveProfileOwnerRecoveryEvidence,
    ) -> Result<ActiveProfileOwnerRecoveryOutcome, PortError> {
        let launch_requirement = match evidence {
            ActiveProfileOwnerRecoveryEvidence::NeverStarted { terminal } => {
                (OwnerRecoveryLaunchRequirement::NeverStarted, terminal)
            }
            ActiveProfileOwnerRecoveryEvidence::ManagedChildExited { context, terminal } => (
                OwnerRecoveryLaunchRequirement::ManagedChildExited(context),
                terminal,
            ),
        };
        let file = ActiveProfileReferenceFile::new(self.config_root.clone());
        let (config_root, _root_guard) = file.acquire_root_lock().await?;
        let file = ActiveProfileReferenceFile::new(config_root.clone());
        let _owner_state_guard = file.acquire_owner_state_lock().await?;
        let mut record = file
            .read_active_profile_record_sync()?
            .unwrap_or_else(ActiveProfileReferenceRecord::pending_legacy_migration);
        let mut proof = file.read_owner_proof_sync()?;
        let no_owner_proof = matches!(proof.state, ActiveCoreOwnerProof::Unowned);
        let already_terminal_without_child_record = matches!(
            (&launch_requirement.0, &proof.state),
            (
                OwnerRecoveryLaunchRequirement::NeverStarted,
                ActiveCoreOwnerProof::Terminal { .. }
            )
        );
        if matches!(record.owner, ActiveCoreOwnerState::Unowned)
            && (no_owner_proof || already_terminal_without_child_record)
        {
            if lock_local_owner_registry()?.contains(&config_root) {
                return Err(transition_rejected(
                    "an active local owner registration exists without a bound owner record",
                ));
            }
            let owner_probe = file.try_acquire_owner_lock()?.ok_or_else(|| {
                transition_rejected(
                    "an active Core owner holds the fence while its persisted record is unowned",
                )
            })?;
            drop(owner_probe);
            return Ok(ActiveProfileOwnerRecoveryOutcome::NoRecoveryRequired);
        }

        let (identity, bound_generation, token, record_position, proof_position) =
            match (record.owner.clone(), proof.state.clone()) {
                (
                    ActiveCoreOwnerState::Bound {
                        identity,
                        generation,
                        token,
                    },
                    ActiveCoreOwnerProof::Bound {
                        identity: proof_identity,
                        generation: proof_generation,
                        token: proof_token,
                    },
                ) if identity.same_instance(&proof_identity)
                    && generation == proof_generation
                    && token == proof_token =>
                {
                    (
                        identity,
                        generation,
                        token,
                        OwnerRecoveryRecordPosition::Bound,
                        OwnerRecoveryProofPosition::Bound,
                    )
                }
                (
                    ActiveCoreOwnerState::Bound {
                        identity,
                        generation,
                        token,
                    },
                    ActiveCoreOwnerProof::Terminal {
                        identity: proof_identity,
                        bound_generation: proof_bound_generation,
                        terminal_generation,
                        token: proof_token,
                    },
                ) if identity.same_instance(&proof_identity)
                    && generation == proof_bound_generation
                    && token == proof_token =>
                {
                    (
                        identity,
                        generation,
                        token,
                        OwnerRecoveryRecordPosition::Bound,
                        OwnerRecoveryProofPosition::Terminal {
                            terminal_generation,
                        },
                    )
                }
                (
                    ActiveCoreOwnerState::Unowned,
                    ActiveCoreOwnerProof::Bound {
                        identity,
                        generation,
                        token,
                    },
                ) => (
                    identity,
                    generation,
                    token,
                    OwnerRecoveryRecordPosition::Unowned,
                    OwnerRecoveryProofPosition::Bound,
                ),
                (
                    ActiveCoreOwnerState::Unowned,
                    ActiveCoreOwnerProof::Terminal {
                        identity,
                        bound_generation,
                        terminal_generation,
                        token,
                    },
                ) => (
                    identity,
                    bound_generation,
                    token,
                    OwnerRecoveryRecordPosition::Unowned,
                    OwnerRecoveryProofPosition::Terminal {
                        terminal_generation,
                    },
                ),
                _ => {
                    return Err(transition_rejected(
                        "no recoverable bound owner proof is present",
                    ));
                }
            };

        let (launch_requirement, terminal) = launch_requirement;
        let terminal_generation =
            validate_terminal_for_owner(&terminal, &identity, bound_generation)?;
        match (&launch_requirement, &proof.launch) {
            (
                OwnerRecoveryLaunchRequirement::NeverStarted,
                ActiveCoreOwnerLaunchProof::Unstarted,
            ) if terminal_generation == CoreGeneration::new(0) => {}
            (
                OwnerRecoveryLaunchRequirement::ManagedChildExited(expected),
                ActiveCoreOwnerLaunchProof::Managed(persisted),
            ) if expected == persisted => {
                let (context_identity, context_generation, session_token) = match expected {
                    CoreProcessStartContext::Managed {
                        identity,
                        generation,
                        session_token,
                    } => (identity, *generation, *session_token),
                    CoreProcessStartContext::Standalone { .. } => {
                        return Err(transition_rejected(
                            "standalone process context cannot recover an active Core owner",
                        ));
                    }
                };
                if !context_identity.same_instance(&identity)
                    || context_generation <= bound_generation
                    || context_generation != terminal_generation
                    || !session_token.is_valid()
                {
                    return Err(transition_rejected(
                        "managed child context does not match exact owner terminal evidence",
                    ));
                }
            }
            _ => {
                return Err(transition_rejected(
                    "owner recovery evidence differs from its persisted launch proof",
                ));
            }
        }
        if matches!(record_position, OwnerRecoveryRecordPosition::Unowned)
            && matches!(proof_position, OwnerRecoveryProofPosition::Bound)
            && terminal_generation != CoreGeneration::new(0)
        {
            return Err(transition_rejected(
                "a partially published owner binding can recover only from a never-started terminal snapshot",
            ));
        }
        if let OwnerRecoveryProofPosition::Terminal {
            terminal_generation: persisted_generation,
        } = proof_position
        {
            if persisted_generation != terminal_generation {
                return Err(transition_rejected(
                    "owner terminal snapshot differs from its persisted exact proof",
                ));
            }
        }
        if local_owner_registration_state(&config_root, &token)?.is_some() {
            return Err(transition_rejected(
                "owner is still registered in this process; use its live owner lease",
            ));
        }
        let owner_probe = file.try_acquire_owner_lock()?.ok_or_else(|| {
            transition_rejected("owner process still holds the active owner fence")
        })?;
        drop(owner_probe);

        if matches!(proof_position, OwnerRecoveryProofPosition::Bound) {
            proof.state = ActiveCoreOwnerProof::Terminal {
                identity: identity.clone(),
                bound_generation,
                terminal_generation,
                token: token.clone(),
            };
            file.publish_owner_proof_sync_reconciled(&proof)?;
        }
        if matches!(record_position, OwnerRecoveryRecordPosition::Bound) {
            record.owner = ActiveCoreOwnerState::Unowned;
            file.publish_active_profile_record_sync_reconciled(&record)?;
        }
        Ok(ActiveProfileOwnerRecoveryOutcome::Recovered(
            record.selection,
        ))
    }

    pub async fn normalize_owner_state_unlocked(
        &self,
        record: &mut ActiveProfileReferenceRecord,
    ) -> Result<(ActiveCoreOwnerState, ThreadedInstanceLockGuard), PortError> {
        if matches!(&record.owner, ActiveCoreOwnerState::Switching { .. }) {
            return Err(transition_rejected(
                "an active Core owner switch is in progress",
            ));
        }

        let file = ActiveProfileReferenceFile::new(self.config_root.clone());
        let owner_state_guard = file.acquire_owner_state_lock().await?;
        let mut normalized = record.clone();
        let owner_proof = file.read_owner_proof_sync()?;
        let owner_probe = file.try_acquire_owner_lock()?;
        let owner_state = match (normalized.owner.clone(), owner_probe) {
            (ActiveCoreOwnerState::Unowned, Some(probe)) => {
                drop(probe);
                match &owner_proof.state {
                    ActiveCoreOwnerProof::Unowned | ActiveCoreOwnerProof::Terminal { .. } => {
                        ActiveCoreOwnerState::Unowned
                    }
                    ActiveCoreOwnerProof::Bound { .. } | ActiveCoreOwnerProof::Reserved { .. } => {
                        return Err(transition_rejected(
                            "unowned active reference has an orphaned live-owner proof; explicit terminal recovery is required",
                        ));
                    }
                }
            }
            (ActiveCoreOwnerState::Unowned, None) => {
                return Err(transition_rejected(
                    "active Core owner lock is held without a persisted owner binding",
                ));
            }
            (
                ActiveCoreOwnerState::Bound {
                    identity,
                    generation,
                    token,
                },
                None,
            ) => match &owner_proof.state {
                ActiveCoreOwnerProof::Bound {
                    identity: proof_identity,
                    generation: proof_generation,
                    token: proof_token,
                } if proof_identity.same_instance(&identity)
                    && proof_generation == &generation
                    && proof_token == &token =>
                {
                    normalized.owner.clone()
                }
                _ => {
                    return Err(transition_rejected(
                        "active Core owner lock and persisted owner binding disagree",
                    ));
                }
            },
            (ActiveCoreOwnerState::Bound { .. }, Some(probe)) => {
                drop(probe);
                match &owner_proof.state {
                    ActiveCoreOwnerProof::Terminal {
                        identity: proof_identity,
                        bound_generation: proof_bound_generation,
                        terminal_generation,
                        token: proof_token,
                    } if matches!(
                        &normalized.owner,
                        ActiveCoreOwnerState::Bound {
                            identity,
                            generation,
                            token,
                        } if proof_identity.same_instance(identity)
                            && proof_bound_generation == generation
                            && proof_token == token
                            && terminal_generation >= generation
                    ) =>
                    {
                        normalized.owner = ActiveCoreOwnerState::Unowned;
                        file.publish_active_profile_record_sync_reconciled(&normalized)?;
                        ActiveCoreOwnerState::Unowned
                    }
                    ActiveCoreOwnerProof::Bound { .. } => {
                        return Err(transition_rejected(
                            "active Core owner exited without exact terminal proof; explicit platform recovery is required",
                        ));
                    }
                    ActiveCoreOwnerProof::Unowned
                    | ActiveCoreOwnerProof::Terminal { .. }
                    | ActiveCoreOwnerProof::Reserved { .. } => {
                        return Err(transition_rejected(
                            "active Core owner record and terminal proof disagree",
                        ));
                    }
                }
            }
            (ActiveCoreOwnerState::Switching { .. }, _) => {
                return Err(transition_rejected(
                    "an active Core owner switch is in progress",
                ));
            }
        };
        *record = normalized;
        Ok((owner_state, owner_state_guard))
    }
}

pub fn validate_reference_write_authorization(
    owner_state: &ActiveCoreOwnerState,
    authorization: &ActiveProfileReferenceWriteAuthorization,
    config_root: &std::path::Path,
) -> Result<(), PortError> {
    match (owner_state, authorization) {
        (ActiveCoreOwnerState::Unowned, ActiveProfileReferenceWriteAuthorization::Unowned) => {
            Ok(())
        }
        (
            ActiveCoreOwnerState::Bound {
                identity,
                generation,
                token,
            },
            ActiveProfileReferenceWriteAuthorization::BoundOwner {
                identity: authorized_identity,
                generation: authorized_generation,
            },
        ) if identity.same_instance(authorized_identity) && generation == authorized_generation => {
            if local_owner_registration_matches(
                config_root,
                token,
                &LocalOwnerRegistrationState::Bound {
                    identity: identity.clone(),
                    generation: *generation,
                },
            )? {
                Ok(())
            } else {
                Err(transition_rejected(
                    "bound-owner reference writes require this process's exact owner lease",
                ))
            }
        }
        (ActiveCoreOwnerState::Switching { .. }, _) => Err(transition_rejected(
            "active profile reference writes are blocked during owner switching",
        )),
        _ => Err(transition_rejected(
            "active profile reference write authorization does not match the owner fence",
        )),
    }
}

fn validate_target_evidence(
    target: &ActiveProfileReferenceTarget,
    local_engine: &EngineId,
    evidence: ActiveProfileReferenceTargetEvidence,
) -> Result<(), PortError> {
    match (target, evidence) {
        (
            ActiveProfileReferenceTarget::Unselected,
            ActiveProfileReferenceTargetEvidence::Unselected,
        ) => Ok(()),
        (
            ActiveProfileReferenceTarget::Selected {
                engine_id,
                profile_id,
            },
            ActiveProfileReferenceTargetEvidence::LocalWorkspace(identity),
        ) if engine_id == local_engine
            && identity.document.engine_id == *engine_id
            && identity.document.profile_id == *profile_id =>
        {
            Ok(())
        }
        (
            ActiveProfileReferenceTarget::Selected { engine_id, .. },
            ActiveProfileReferenceTargetEvidence::OtherEngine,
        ) if engine_id != local_engine => Ok(()),
        _ => Err(PortError::Rejected(Failure::new(
            ErrorCode::NotReady,
            "selected profile document identity differs from the active reference target",
            false,
        ))),
    }
}

pub fn validate_terminal_for_owner(
    terminal: &CoreLifecycleSnapshot,
    identity: &CoreIdentity,
    minimum_generation: CoreGeneration,
) -> Result<CoreGeneration, PortError> {
    if !terminal
        .identity
        .as_ref()
        .is_some_and(|terminal_identity| identity.same_instance(terminal_identity))
        || terminal.generation < minimum_generation.value()
        || terminal.lifecycle != CoreLifecycle::Stopped
        || terminal.session_token.is_some()
    {
        return Err(transition_rejected(
            "active Core owner switch requires matching Stopped readback without a session",
        ));
    }
    Ok(CoreGeneration::new(terminal.generation))
}

#[cfg(test)]
#[path = "active_reference_owner_coordinator_tests.rs"]
mod tests;
