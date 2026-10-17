//! Durable owner-switch lease transitions shared by all engine stores.

use crate::active_owner_lease::ActiveCoreOwnerLease;
use crate::active_owner_registry::{
    LocalOwnerRegistrationState, LocalSwitchRegistrationRecovery, local_owner_registration_matches,
    local_owner_registration_state, lock_local_owner_registry, remove_local_owner_registration,
};
use crate::active_reference_file::ActiveProfileReferenceFile;
use crate::active_reference_owner::{
    ActiveCoreOwnerLaunchProof, ActiveCoreOwnerProof, ActiveCoreOwnerProofRecord,
    ActiveCoreOwnerState, ActiveOwnerSwitchStage, ActiveOwnerToken, ActiveProfileReferenceRecord,
    LegacySelectionMigration, transition_rejected,
};
use crate::active_reference_owner_coordinator::{
    ActiveProfileOwnerCoordinator, validate_terminal_for_owner,
};
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReference, ActiveProfileReferenceRevision, ActiveProfileReferenceState,
    ActiveProfileReferenceTarget,
};
use infiltrator_platform::instance_lock::ThreadedInstanceLockGuard;
use infiltrator_ports::active_profile_reference_store::{
    ActiveProfileOwnerBindingLease, ActiveProfileOwnerLease, ActiveProfileOwnerStoppedSwitchLease,
    ActiveProfileOwnerSwitchLease, ActiveProfileOwnerSwitchRecovery, ActiveProfileOwnerSwitchState,
    ActiveProfileReferenceChangedLease,
};
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::{CoreGeneration, CoreIdentity, EngineId};
use lurker_core_contract::lifecycle::{CoreLifecycle, CoreLifecycleSnapshot};
use lurker_core_ports::error::PortError;
use std::path::{Path, PathBuf};

fn sync_read_active_profile_record(
    config_root: &Path,
) -> Result<ActiveProfileReferenceRecord, PortError> {
    Ok(ActiveProfileReferenceFile::new(config_root)
        .read_active_profile_record_sync()?
        .unwrap_or_else(ActiveProfileReferenceRecord::pending_legacy_migration))
}

fn sync_read_active_core_owner_proof(
    config_root: &Path,
) -> Result<ActiveCoreOwnerProofRecord, PortError> {
    ActiveProfileReferenceFile::new(config_root).read_owner_proof_sync()
}

fn sync_publish_active_core_owner_proof(
    config_root: &Path,
    record: &ActiveCoreOwnerProofRecord,
) -> Result<(), PortError> {
    ActiveProfileReferenceFile::new(config_root).publish_owner_proof_sync_reconciled(record)
}

fn sync_publish_active_profile_record(
    config_root: &Path,
    record: &ActiveProfileReferenceRecord,
) -> Result<(), PortError> {
    ActiveProfileReferenceFile::new(config_root)
        .publish_active_profile_record_sync_reconciled(record)
}

pub fn switching_state(
    record: &ActiveProfileReferenceRecord,
    transition_id: &ActiveOwnerToken,
) -> Result<
    (
        ActiveProfileReferenceState,
        CoreIdentity,
        CoreGeneration,
        ActiveOwnerToken,
        ActiveOwnerSwitchStage,
    ),
    PortError,
> {
    match &record.owner {
        ActiveCoreOwnerState::Switching {
            transition_id: persisted_id,
            previous_reference,
            owner_identity,
            owner_generation,
            owner_token,
            stage,
        } if persisted_id == transition_id => Ok((
            previous_reference.clone(),
            owner_identity.clone(),
            *owner_generation,
            owner_token.clone(),
            stage.clone(),
        )),
        _ => Err(transition_rejected(
            "active Core owner switch reservation is stale or no longer present",
        )),
    }
}

pub fn terminal_proof_generation(
    proof: &ActiveCoreOwnerProofRecord,
    identity: &CoreIdentity,
    bound_generation: CoreGeneration,
    token: &ActiveOwnerToken,
) -> Result<CoreGeneration, PortError> {
    match &proof.state {
        ActiveCoreOwnerProof::Terminal {
            identity: proof_identity,
            bound_generation: proof_bound_generation,
            terminal_generation,
            token: proof_token,
        } if proof_identity.same_instance(identity)
            && *proof_bound_generation == bound_generation
            && proof_token == token
            && *terminal_generation >= bound_generation =>
        {
            Ok(*terminal_generation)
        }
        _ => Err(transition_rejected(
            "persisted owner terminal proof does not match the switch reservation",
        )),
    }
}

pub fn recover_local_switch_registration(
    config_root: &Path,
    token: &ActiveOwnerToken,
    transition_id: &ActiveOwnerToken,
    identity: &CoreIdentity,
    generation: CoreGeneration,
) -> Result<LocalSwitchRegistrationRecovery, PortError> {
    let mut registrations = lock_local_owner_registry()?;
    Ok(registrations.recover_switch(config_root, token, transition_id, identity, generation))
}

#[path = "active_owner_switch/switch_lease.rs"]
mod switch_lease;
use switch_lease::*;

#[path = "active_owner_switch/stopped_lease.rs"]
mod stopped_lease;
use stopped_lease::*;

#[path = "active_owner_switch/changed_lease.rs"]
mod changed_lease;
use changed_lease::*;

#[path = "active_owner_switch/binding_lease.rs"]
mod binding_lease;
use binding_lease::*;

impl ActiveProfileOwnerCoordinator {
    /// Begin a switch while the caller holds the root fence and has checked
    /// the product reset fence.
    pub async fn begin_owner_switch_while_root_locked(
        &self,
        root_guard: ThreadedInstanceLockGuard,
        expected_revision: ActiveProfileReferenceRevision,
        expected_owner: CoreIdentity,
        expected_binding_generation: CoreGeneration,
    ) -> Result<Box<dyn ActiveProfileOwnerSwitchLease>, PortError> {
        let _root_guard = root_guard;
        let config_root = self.config_root().to_path_buf();
        let mut record = sync_read_active_profile_record(&config_root)?;
        let _owner_state_guard = ActiveProfileReferenceFile::new(config_root.clone())
            .acquire_owner_state_lock()
            .await?;
        if record.selection.revision() != expected_revision {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "active profile reference revision is stale",
                true,
            )));
        }
        let (owner_identity, owner_generation, owner_token) = match &record.owner {
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
        if ActiveProfileReferenceFile::new(config_root.clone())
            .try_acquire_owner_lock()?
            .is_some()
        {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "persisted owner binding has no live kernel owner lock",
                true,
            )));
        }
        let proof = sync_read_active_core_owner_proof(&config_root)?;
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
        let previous_reference = record.selection.clone();
        record.owner = ActiveCoreOwnerState::Switching {
            transition_id: transition_id.clone(),
            previous_reference: previous_reference.clone(),
            owner_identity: owner_identity.clone(),
            owner_generation,
            owner_token: owner_token.clone(),
            stage: ActiveOwnerSwitchStage::ClosingOld,
        };
        sync_publish_active_profile_record(&config_root, &record)?;
        lock_local_owner_registry()?.begin_switching(
            &config_root,
            &owner_token,
            transition_id.clone(),
            owner_identity.clone(),
            owner_generation,
        )?;
        let state = ActiveProfileOwnerSwitchState::ClosingOld {
            previous_reference,
            owner_identity,
            owner_generation,
        };
        Ok(Box::new(ActiveOwnerSwitchLease::new(
            config_root,
            transition_id,
            expected_owner,
            expected_binding_generation,
            state,
        )))
    }
}

impl ActiveProfileOwnerCoordinator {
    pub async fn recover_owner_switch(
        &self,
    ) -> Result<ActiveProfileOwnerSwitchRecovery, PortError> {
        let (config_root, _root_guard) =
            ActiveProfileReferenceFile::new(self.config_root().to_path_buf())
                .acquire_root_lock()
                .await?;
        let _owner_state_guard = ActiveProfileReferenceFile::new(config_root.clone())
            .acquire_owner_state_lock()
            .await?;
        let mut record = sync_read_active_profile_record(&config_root)?;
        let (
            transition_id,
            previous_reference,
            owner_identity,
            owner_generation,
            owner_token,
            stage,
        ) = match &record.owner {
            ActiveCoreOwnerState::Switching {
                transition_id,
                previous_reference,
                owner_identity,
                owner_generation,
                owner_token,
                stage,
            } => (
                transition_id.clone(),
                previous_reference.clone(),
                owner_identity.clone(),
                *owner_generation,
                owner_token.clone(),
                stage.clone(),
            ),
            ActiveCoreOwnerState::Unowned | ActiveCoreOwnerState::Bound { .. } => {
                return Ok(ActiveProfileOwnerSwitchRecovery::NoPendingSwitch);
            }
        };
        match stage {
            ActiveOwnerSwitchStage::ClosingOld => {
                match ActiveProfileReferenceFile::new(config_root.clone())
                    .try_acquire_owner_lock()?
                {
                    None => {
                        let local_recovery = recover_local_switch_registration(
                            &config_root,
                            &owner_token,
                            &transition_id,
                            &owner_identity,
                            owner_generation,
                        )?;
                        if !matches!(
                            local_recovery,
                            LocalSwitchRegistrationRecovery::NotHeldByThisOwner
                        ) {
                            let state = ActiveProfileOwnerSwitchState::ClosingOld {
                                previous_reference,
                                owner_identity: owner_identity.clone(),
                                owner_generation,
                            };
                            return Ok(ActiveProfileOwnerSwitchRecovery::ResumeClosingOld(
                                Box::new(ActiveOwnerSwitchLease::new(
                                    config_root,
                                    transition_id,
                                    owner_identity,
                                    owner_generation,
                                    state,
                                )),
                            ));
                        }
                        Ok(ActiveProfileOwnerSwitchRecovery::OwnerStillLive {
                            identity: owner_identity,
                            generation: owner_generation,
                        })
                    }
                    Some(owner_probe) => {
                        drop(owner_probe);
                        let proof = sync_read_active_core_owner_proof(&config_root)?;
                        match terminal_proof_generation(
                            &proof,
                            &owner_identity,
                            owner_generation,
                            &owner_token,
                        ) {
                            Ok(terminal_generation) => {
                                record.owner = ActiveCoreOwnerState::Switching {
                                    transition_id: transition_id.clone(),
                                    previous_reference: previous_reference.clone(),
                                    owner_identity: owner_identity.clone(),
                                    owner_generation,
                                    owner_token,
                                    stage: ActiveOwnerSwitchStage::OldStopped {
                                        terminal_generation,
                                    },
                                };
                                sync_publish_active_profile_record(&config_root, &record)?;
                                let state = ActiveProfileOwnerSwitchState::OldStopped {
                                    previous_reference,
                                    owner_identity,
                                    owner_generation,
                                    terminal_generation,
                                };
                                Ok(ActiveProfileOwnerSwitchRecovery::ResumeOldStopped(
                                    Box::new(ActiveOwnerStoppedSwitchLease::new(
                                        config_root,
                                        transition_id,
                                        state,
                                    )),
                                ))
                            }
                            Err(_) => Ok(
                                ActiveProfileOwnerSwitchRecovery::OwnerTerminalEvidenceRequired {
                                    identity: owner_identity,
                                    bound_generation: owner_generation,
                                },
                            ),
                        }
                    }
                }
            }
            ActiveOwnerSwitchStage::OldStopped {
                terminal_generation,
            } => {
                let proof = sync_read_active_core_owner_proof(&config_root)?;
                let proven_generation = terminal_proof_generation(
                    &proof,
                    &owner_identity,
                    owner_generation,
                    &owner_token,
                )?;
                if proven_generation != terminal_generation {
                    return Err(transition_rejected(
                        "persisted old-owner terminal generation differs from its proof",
                    ));
                }
                if ActiveProfileReferenceFile::new(config_root.clone())
                    .try_acquire_owner_lock()?
                    .is_none()
                {
                    return Ok(ActiveProfileOwnerSwitchRecovery::OwnerStillLive {
                        identity: owner_identity,
                        generation: owner_generation,
                    });
                }
                let state = ActiveProfileOwnerSwitchState::OldStopped {
                    previous_reference,
                    owner_identity,
                    owner_generation,
                    terminal_generation,
                };
                Ok(ActiveProfileOwnerSwitchRecovery::ResumeOldStopped(
                    Box::new(ActiveOwnerStoppedSwitchLease::new(
                        config_root,
                        transition_id,
                        state,
                    )),
                ))
            }
            ActiveOwnerSwitchStage::ReferenceChanged {
                terminal_generation,
            } => {
                let owner_probe = ActiveProfileReferenceFile::new(config_root.clone())
                    .try_acquire_owner_lock()?;
                let Some(owner_probe) = owner_probe else {
                    return Ok(ActiveProfileOwnerSwitchRecovery::OwnerStillLive {
                        identity: owner_identity,
                        generation: owner_generation,
                    });
                };
                drop(owner_probe);
                let mut proof = sync_read_active_core_owner_proof(&config_root)?;
                let proven_generation = match terminal_proof_generation(
                    &proof,
                    &owner_identity,
                    owner_generation,
                    &owner_token,
                ) {
                    Ok(proven_generation) => proven_generation,
                    Err(_) => {
                        // `reserve_new_owner` persists its kernel admission
                        // proof before advancing the root record to
                        // BindingNew. If a process dies between those writes,
                        // this ReferenceChanged stage proves provider
                        // dispatch never received a lease. Restore the old
                        // terminal proof so the durable CAS stage can resume.
                        let selected_engine = match &record.selection {
                            ActiveProfileReferenceState::Selected(reference) => {
                                &reference.engine_id
                            }
                            ActiveProfileReferenceState::Unselected { .. } => {
                                return Err(transition_rejected(
                                    "reference-changed owner switch has no selected engine",
                                ));
                            }
                        };
                        if !matches!(
                            &proof.state,
                            ActiveCoreOwnerProof::Reserved {
                                transition_id: proof_transition,
                                engine_id: proof_engine,
                                ..
                            } if proof_transition == &transition_id
                                && proof_engine == selected_engine
                        ) {
                            return Err(transition_rejected(
                                "persisted owner terminal generation differs from its proof",
                            ));
                        }
                        proof.state = ActiveCoreOwnerProof::Terminal {
                            identity: owner_identity.clone(),
                            bound_generation: owner_generation,
                            terminal_generation,
                            token: owner_token.clone(),
                        };
                        sync_publish_active_core_owner_proof(&config_root, &proof)?;
                        terminal_generation
                    }
                };
                if proven_generation != terminal_generation {
                    return Err(transition_rejected(
                        "persisted owner terminal generation differs from its proof",
                    ));
                }
                let state = ActiveProfileOwnerSwitchState::ReferenceChanged {
                    previous_reference,
                    selection: record.selection,
                    owner_identity,
                    terminal_generation,
                };
                Ok(ActiveProfileOwnerSwitchRecovery::ResumeReferenceChanged(
                    Box::new(ReferenceChangedLease::new(
                        config_root,
                        transition_id,
                        state,
                    )),
                ))
            }
            ActiveOwnerSwitchStage::BindingNew {
                engine_id, token, ..
            } => match ActiveProfileReferenceFile::new(config_root.clone())
                .try_acquire_owner_lock()?
            {
                None => {
                    let local_state = LocalOwnerRegistrationState::Binding {
                        transition_id: transition_id.clone(),
                    };
                    if local_owner_registration_matches(&config_root, &token, &local_state)? {
                        let state = ActiveProfileOwnerSwitchState::BindingNew {
                            selection: record.selection,
                            engine_id,
                        };
                        Ok(ActiveProfileOwnerSwitchRecovery::ResumeBindingNew(
                            Box::new(OwnerBindingLease::new(
                                config_root,
                                transition_id,
                                token,
                                state,
                            )),
                        ))
                    } else {
                        Ok(ActiveProfileOwnerSwitchRecovery::CandidateStillLive { engine_id })
                    }
                }
                Some(owner_probe) => {
                    drop(owner_probe);
                    Ok(
                        ActiveProfileOwnerSwitchRecovery::CandidateTerminalEvidenceRequired {
                            selection: record.selection,
                            engine_id,
                        },
                    )
                }
            },
        }
    }

    pub async fn recover_old_owner_after_terminal(
        &self,
        terminal: &CoreLifecycleSnapshot,
    ) -> Result<Box<dyn ActiveProfileOwnerStoppedSwitchLease>, PortError> {
        let (config_root, _root_guard) =
            ActiveProfileReferenceFile::new(self.config_root().to_path_buf())
                .acquire_root_lock()
                .await?;
        let _owner_state_guard = ActiveProfileReferenceFile::new(config_root.clone())
            .acquire_owner_state_lock()
            .await?;
        let mut record = sync_read_active_profile_record(&config_root)?;
        let (transition_id, previous_reference, owner_identity, owner_generation, owner_token) =
            match &record.owner {
                ActiveCoreOwnerState::Switching {
                    transition_id,
                    previous_reference,
                    owner_identity,
                    owner_generation,
                    owner_token,
                    stage: ActiveOwnerSwitchStage::ClosingOld,
                } => (
                    transition_id.clone(),
                    previous_reference.clone(),
                    owner_identity.clone(),
                    *owner_generation,
                    owner_token.clone(),
                ),
                _ => {
                    return Err(transition_rejected(
                        "old-owner terminal recovery requires a ClosingOld reservation",
                    ));
                }
            };
        if record.selection != previous_reference {
            return Err(transition_rejected(
                "active reference changed before old-owner terminal recovery",
            ));
        }
        let terminal_generation =
            validate_terminal_for_owner(terminal, &owner_identity, owner_generation)?;
        if local_owner_registration_state(&config_root, &owner_token)?.is_some() {
            return Err(transition_rejected(
                "old owner still has a local registration; recover its live switch lease",
            ));
        }
        let owner_probe = ActiveProfileReferenceFile::new(config_root.clone())
            .try_acquire_owner_lock()?
            .ok_or_else(|| {
                transition_rejected("old owner still holds the cross-process owner fence")
            })?;
        drop(owner_probe);

        let mut proof = sync_read_active_core_owner_proof(&config_root)?;
        match proof.state.clone() {
            ActiveCoreOwnerProof::Bound {
                identity: proof_identity,
                generation: proof_generation,
                token: proof_token,
            } if proof_identity.same_instance(&owner_identity)
                && proof_generation == owner_generation
                && proof_token == owner_token =>
            {
                proof.state = ActiveCoreOwnerProof::Terminal {
                    identity: owner_identity.clone(),
                    bound_generation: owner_generation,
                    terminal_generation,
                    token: owner_token.clone(),
                };
                sync_publish_active_core_owner_proof(&config_root, &proof)?;
            }
            ActiveCoreOwnerProof::Terminal {
                identity: proof_identity,
                bound_generation: proof_generation,
                terminal_generation: proof_terminal_generation,
                token: proof_token,
            } if proof_identity.same_instance(&owner_identity)
                && proof_generation == owner_generation
                && proof_terminal_generation == terminal_generation
                && proof_token == owner_token => {}
            _ => {
                return Err(transition_rejected(
                    "old-owner terminal snapshot does not match the persisted owner proof",
                ));
            }
        }
        record.owner = ActiveCoreOwnerState::Switching {
            transition_id: transition_id.clone(),
            previous_reference: previous_reference.clone(),
            owner_identity: owner_identity.clone(),
            owner_generation,
            owner_token,
            stage: ActiveOwnerSwitchStage::OldStopped {
                terminal_generation,
            },
        };
        sync_publish_active_profile_record(&config_root, &record)?;
        let state = ActiveProfileOwnerSwitchState::OldStopped {
            previous_reference,
            owner_identity,
            owner_generation,
            terminal_generation,
        };
        Ok(Box::new(ActiveOwnerStoppedSwitchLease::new(
            config_root,
            transition_id,
            state,
        )))
    }

    pub async fn recover_candidate_after_terminal(
        &self,
        terminal: &CoreLifecycleSnapshot,
    ) -> Result<ActiveProfileReferenceState, PortError> {
        let (config_root, _root_guard) =
            ActiveProfileReferenceFile::new(self.config_root().to_path_buf())
                .acquire_root_lock()
                .await?;
        let _owner_state_guard = ActiveProfileReferenceFile::new(config_root.clone())
            .acquire_owner_state_lock()
            .await?;
        let mut record = sync_read_active_profile_record(&config_root)?;
        let (
            transition_id,
            engine_id,
            token,
            old_owner_identity,
            old_owner_generation,
            old_owner_token,
            old_terminal_generation,
        ) = match &record.owner {
            ActiveCoreOwnerState::Switching {
                transition_id,
                owner_identity,
                owner_generation,
                owner_token,
                stage:
                    ActiveOwnerSwitchStage::BindingNew {
                        engine_id,
                        token,
                        terminal_generation,
                    },
                ..
            } => (
                transition_id.clone(),
                engine_id.clone(),
                token.clone(),
                owner_identity.clone(),
                *owner_generation,
                owner_token.clone(),
                *terminal_generation,
            ),
            _ => {
                return Err(transition_rejected(
                    "no crashed candidate reservation is pending",
                ));
            }
        };
        let identity = terminal
            .identity
            .as_ref()
            .ok_or_else(|| transition_rejected("candidate terminal has no Core identity"))?;
        if identity.engine != engine_id
            || terminal.lifecycle != CoreLifecycle::Stopped
            || terminal.generation != 0
            || terminal.session_token.is_some()
        {
            return Err(transition_rejected(
                "candidate recovery requires a never-started Stopped readback without a session",
            ));
        }
        if local_owner_registration_state(&config_root, &token)?.is_some() {
            return Err(transition_rejected(
                "candidate owner is still registered in this process; use its binding lease",
            ));
        }
        if ActiveProfileReferenceFile::new(config_root.clone())
            .try_acquire_owner_lock()?
            .is_none()
        {
            return Err(transition_rejected(
                "candidate process still holds the active owner fence",
            ));
        }
        let mut proof = sync_read_active_core_owner_proof(&config_root)?;
        match &proof.state {
            ActiveCoreOwnerProof::Reserved {
                transition_id: proof_transition,
                engine_id: proof_engine,
                token: proof_token,
            } if proof_transition == &transition_id
                && proof_engine == &engine_id
                && proof_token == &token => {}
            ActiveCoreOwnerProof::Bound {
                identity: proof_identity,
                generation: proof_generation,
                token: proof_token,
            } if proof_identity.same_instance(identity)
                && *proof_generation == CoreGeneration::new(terminal.generation)
                && *proof_generation == CoreGeneration::new(0)
                && proof_token == &token => {}
            ActiveCoreOwnerProof::Terminal {
                identity: proof_identity,
                bound_generation: proof_bound_generation,
                terminal_generation: proof_terminal_generation,
                token: proof_token,
            } if proof_identity.same_instance(&old_owner_identity)
                && *proof_bound_generation == old_owner_generation
                && *proof_terminal_generation == old_terminal_generation
                && proof_token == &old_owner_token => {}
            _ => return Err(transition_rejected("candidate owner proof is stale")),
        }
        proof.state = ActiveCoreOwnerProof::Terminal {
            identity: old_owner_identity,
            bound_generation: old_owner_generation,
            terminal_generation: old_terminal_generation,
            token: old_owner_token,
        };
        sync_publish_active_core_owner_proof(&config_root, &proof)?;
        record.owner = ActiveCoreOwnerState::Unowned;
        sync_publish_active_profile_record(&config_root, &record)?;
        Ok(record.selection)
    }
}
