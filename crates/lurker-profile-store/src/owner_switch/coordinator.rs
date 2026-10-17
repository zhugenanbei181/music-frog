//! Cross-process Core owner normalization and terminal recovery shared by all
//! selection payloads.

use crate::instance_lock::ThreadedInstanceLockGuard;
use crate::owner_switch::codec::OwnerSwitchCodec;
use crate::owner_switch::file::OwnerSwitchFile;
use crate::owner_switch::leases::switch_lease::ActiveOwnerSwitchLease;
use crate::owner_switch::owner::{
    ActiveCoreOwnerLaunchProof, ActiveCoreOwnerProof, ActiveCoreOwnerState, ActiveOwnerSwitchStage,
    ActiveOwnerToken, transition_rejected,
};
use crate::owner_switch::registry::{
    LocalOwnerRegistrationState, local_owner_registration_matches, local_owner_registration_state,
    lock_local_owner_registry,
};
use crate::owner_switch::stage::switch_stage;
use lurker_core_contract::core_process::CoreProcessStartContext;
use lurker_core_contract::engine_switch::EngineSwitchRevision;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::{CoreGeneration, CoreIdentity};
use lurker_core_contract::lifecycle::{CoreLifecycle, CoreLifecycleSnapshot};
use lurker_core_ports::engine_switch::{
    EngineOwnerSwitchLease, OwnerRecoveryEvidence, OwnerRecoveryOutcome,
};
use lurker_core_ports::error::PortError;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

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
/// after it, preserving the root -> owner-state lock order.
pub struct OwnerSwitchCoordinator<C: OwnerSwitchCodec> {
    config_root: PathBuf,
    codec: PhantomData<C>,
}

impl<C: OwnerSwitchCodec> OwnerSwitchCoordinator<C> {
    pub fn new(config_root: impl Into<PathBuf>) -> Self {
        Self {
            config_root: config_root.into(),
            codec: PhantomData,
        }
    }

    pub fn config_root(&self) -> &Path {
        &self.config_root
    }

    /// Reconcile a persisted owner only after the host has supplied exact
    /// process-exit evidence and a matching terminal lifecycle snapshot.
    pub async fn recover_bound_owner_after_terminal(
        &self,
        evidence: OwnerRecoveryEvidence,
    ) -> Result<OwnerRecoveryOutcome<C::Selection>, PortError> {
        let launch_requirement = match evidence {
            OwnerRecoveryEvidence::NeverStarted { terminal } => {
                (OwnerRecoveryLaunchRequirement::NeverStarted, terminal)
            }
            OwnerRecoveryEvidence::ManagedChildExited { context, terminal } => (
                OwnerRecoveryLaunchRequirement::ManagedChildExited(context),
                terminal,
            ),
        };
        let file = OwnerSwitchFile::<C>::new(self.config_root.clone());
        let (config_root, _root_guard) = file.acquire_root_lock().await?;
        let file = OwnerSwitchFile::<C>::new(config_root.clone());
        let _owner_state_guard = file.acquire_owner_state_lock().await?;
        let mut record = file
            .read_active_profile_record_sync()?
            .unwrap_or_else(C::pending);
        let mut proof = file.read_owner_proof_sync()?;
        let no_owner_proof = matches!(proof.state, ActiveCoreOwnerProof::Unowned);
        let already_terminal_without_child_record = matches!(
            (&launch_requirement.0, &proof.state),
            (
                OwnerRecoveryLaunchRequirement::NeverStarted,
                ActiveCoreOwnerProof::Terminal { .. }
            )
        );
        if matches!(C::owner(&record), ActiveCoreOwnerState::Unowned)
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
            return Ok(OwnerRecoveryOutcome::NoRecoveryRequired);
        }

        let (identity, bound_generation, token, record_position, proof_position) =
            match (C::owner(&record).clone(), proof.state.clone()) {
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
            C::set_owner(&mut record, ActiveCoreOwnerState::Unowned);
            file.publish_active_profile_record_sync_reconciled(&record)?;
        }
        Ok(OwnerRecoveryOutcome::Recovered(
            C::selection(&record).clone(),
        ))
    }

    pub async fn normalize_owner_state_unlocked(
        &self,
        record: &mut C::Record,
    ) -> Result<
        (
            ActiveCoreOwnerState<C::Selection>,
            ThreadedInstanceLockGuard,
        ),
        PortError,
    > {
        if matches!(C::owner(record), ActiveCoreOwnerState::Switching { .. }) {
            return Err(transition_rejected(
                "an active Core owner switch is in progress",
            ));
        }

        let file = OwnerSwitchFile::<C>::new(self.config_root.clone());
        let owner_state_guard = file.acquire_owner_state_lock().await?;
        let mut normalized = record.clone();
        let owner_proof = file.read_owner_proof_sync()?;
        let owner_probe = file.try_acquire_owner_lock()?;
        let owner_state = match (C::owner(&normalized).clone(), owner_probe) {
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
                    C::owner(&normalized).clone()
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
                        C::owner(&normalized),
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
                        C::set_owner(&mut normalized, ActiveCoreOwnerState::Unowned);
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

    /// Begin a switch while the caller holds the root fence and has checked
    /// the product reset fence.
    pub async fn begin_owner_switch_while_root_locked(
        &self,
        root_guard: ThreadedInstanceLockGuard,
        expected_revision: EngineSwitchRevision,
        previous: C::Selection,
        expected_owner: CoreIdentity,
        expected_binding_generation: CoreGeneration,
    ) -> Result<Box<dyn EngineOwnerSwitchLease<C::Selection>>, PortError> {
        let _root_guard = root_guard;
        let config_root = self.config_root.to_path_buf();
        let mut record = OwnerSwitchFile::<C>::new(config_root.clone())
            .read_active_profile_record_sync_or_pending()?;
        let _owner_state_guard = OwnerSwitchFile::<C>::new(config_root.clone())
            .acquire_owner_state_lock()
            .await?;
        if C::selection_revision(C::selection(&record)) != expected_revision
            || C::selection(&record) != &previous
        {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "active profile reference revision is stale",
                true,
            )));
        }
        let (owner_identity, owner_generation, owner_token) = match C::owner(&record) {
            ActiveCoreOwnerState::Bound {
                identity,
                generation,
                token,
            } if identity.same_instance(&expected_owner)
                && *generation == expected_binding_generation =>
            {
                (identity.clone(), *generation, token.clone())
            }
            ActiveCoreOwnerState::Bound {
                identity,
                generation,
                ..
            } => {
                return Err(PortError::Rejected(Failure::new(
                    ErrorCode::NotReady,
                    format!(
                        "active Core owner {}/{} at generation {} does not match switch request",
                        identity.engine.as_str(),
                        identity.instance.as_str(),
                        generation.value()
                    ),
                    true,
                )));
            }
            ActiveCoreOwnerState::Unowned => {
                return Err(PortError::Rejected(Failure::new(
                    ErrorCode::NotReady,
                    "owner switching requires a currently bound Core owner",
                    true,
                )));
            }
            ActiveCoreOwnerState::Switching { .. } => {
                return Err(PortError::Rejected(Failure::new(
                    ErrorCode::NotReady,
                    "an active Core owner switch is already in progress",
                    true,
                )));
            }
        };
        if OwnerSwitchFile::<C>::new(config_root.clone())
            .try_acquire_owner_lock()?
            .is_some()
        {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "persisted owner binding has no live kernel owner lock",
                true,
            )));
        }
        let proof = OwnerSwitchFile::<C>::new(config_root.clone()).read_owner_proof_sync()?;
        if !matches!(
            proof.state,
            ActiveCoreOwnerProof::Bound {
                ref identity,
                generation,
                ref token,
            } if identity.same_instance(&owner_identity)
                && generation == owner_generation
                && token == &owner_token
        ) {
            return Err(transition_rejected(
                "persisted owner proof does not match the active owner binding",
            ));
        }
        if !local_owner_registration_matches(
            &config_root,
            &owner_token,
            &LocalOwnerRegistrationState::Bound {
                identity: owner_identity.clone(),
                generation: owner_generation,
            },
        )? {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "only the process holding the old Core owner can reserve its switch",
                true,
            )));
        }
        let transition_id = ActiveOwnerToken::generate()?;
        let previous_reference = C::selection(&record).clone();
        C::set_owner(
            &mut record,
            ActiveCoreOwnerState::Switching {
                transition_id: transition_id.clone(),
                previous_reference: previous_reference.clone(),
                owner_identity: owner_identity.clone(),
                owner_generation,
                owner_token: owner_token.clone(),
                stage: ActiveOwnerSwitchStage::ClosingOld,
            },
        );
        OwnerSwitchFile::<C>::new(config_root.clone())
            .publish_active_profile_record_sync_reconciled(&record)?;
        lock_local_owner_registry()?.begin_switching(
            &config_root,
            &owner_token,
            transition_id.clone(),
            owner_identity.clone(),
            owner_generation,
        )?;
        let stage = switch_stage(
            &ActiveOwnerSwitchStage::ClosingOld,
            &previous_reference,
            &previous_reference,
            &owner_identity,
            owner_generation,
        );
        Ok(Box::new(ActiveOwnerSwitchLease::<C>::new(
            config_root,
            transition_id,
            owner_token,
            stage,
        )))
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
