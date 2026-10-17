//! Restart-visible switch recovery.
//!
//! Observation and lease rehydration are separate, exactly like the neutral
//! port: [`recover_owner_switch`] reports the durable stage without inventing
//! process evidence, and the lease-producing entry points run only after the
//! caller holds an exact terminal generation.

use super::*;
use crate::active_owner_switch::stage_mapping::{
    active_reference_revision, switch_snapshot, switch_stage,
};
use lurker_core_contract::engine_switch::{
    EngineSwitchRecovery, EngineSwitchRevision, EngineSwitchSnapshot,
};

pub(crate) async fn recover_owner_switch(
    config_root: PathBuf,
    expected_revision: EngineSwitchRevision,
) -> Result<EngineSwitchRecovery<ActiveProfileReferenceState>, PortError> {
    let (config_root, _root_guard) = ActiveProfileReferenceFile::new(config_root)
        .acquire_root_lock()
        .await?;
    let _owner_state_guard = ActiveProfileReferenceFile::new(config_root.clone())
        .acquire_owner_state_lock()
        .await?;
    let record = sync_read_active_profile_record(&config_root)?;
    if record.selection.revision() != active_reference_revision(expected_revision) {
        return Err(PortError::Rejected(Failure::new(
            ErrorCode::NotReady,
            "active profile reference revision is stale",
            true,
        )));
    }
    let (transition_id, owner_identity, owner_generation, owner_token, stage) = match &record.owner
    {
        ActiveCoreOwnerState::Unowned | ActiveCoreOwnerState::Bound { .. } => {
            return Ok(EngineSwitchRecovery::NoPendingSwitch);
        }
        ActiveCoreOwnerState::Switching {
            transition_id,
            owner_identity,
            owner_generation,
            owner_token,
            stage,
            ..
        } => (
            transition_id.clone(),
            owner_identity.clone(),
            *owner_generation,
            owner_token.clone(),
            stage.clone(),
        ),
    };
    match &stage {
        ActiveOwnerSwitchStage::ClosingOld => {}
        ActiveOwnerSwitchStage::OldStopped {
            terminal_generation,
        } => {
            let proof = sync_read_active_core_owner_proof(&config_root)?;
            let proven =
                terminal_proof_generation(&proof, &owner_identity, owner_generation, &owner_token)?;
            if proven != *terminal_generation {
                return Err(transition_rejected(
                    "persisted old-owner terminal generation differs from its proof",
                ));
            }
        }
        ActiveOwnerSwitchStage::ReferenceChanged {
            terminal_generation,
        } => {
            repair_candidate_reservation_proof(
                &config_root,
                &record,
                &transition_id,
                &owner_identity,
                owner_generation,
                &owner_token,
                *terminal_generation,
            )?;
        }
        ActiveOwnerSwitchStage::BindingNew { .. } => {}
    }
    let neutral = switch_stage(
        &stage,
        match &record.owner {
            ActiveCoreOwnerState::Switching {
                previous_reference, ..
            } => previous_reference,
            ActiveCoreOwnerState::Unowned | ActiveCoreOwnerState::Bound { .. } => {
                unreachable!("stage was matched as switching")
            }
        },
        &record.selection,
        &owner_identity,
        owner_generation,
    );
    Ok(EngineSwitchRecovery::from(&neutral))
}

/// `reserve_new_owner` persists its kernel admission proof before advancing the
/// root record to `BindingNew`. A process death between those writes leaves a
/// `ReferenceChanged` stage whose proof is already `Reserved`; provider
/// dispatch never received a lease, so restoring the old terminal proof is the
/// exact inverse and lets the durable CAS stage resume.
#[allow(clippy::too_many_arguments)]
fn repair_candidate_reservation_proof(
    config_root: &Path,
    record: &ActiveProfileReferenceRecord,
    transition_id: &ActiveOwnerToken,
    owner_identity: &CoreIdentity,
    owner_generation: CoreGeneration,
    owner_token: &ActiveOwnerToken,
    terminal_generation: CoreGeneration,
) -> Result<(), PortError> {
    let mut proof = sync_read_active_core_owner_proof(config_root)?;
    if let Ok(proven) =
        terminal_proof_generation(&proof, owner_identity, owner_generation, owner_token)
    {
        if proven != terminal_generation {
            return Err(transition_rejected(
                "persisted old-owner terminal generation differs from its proof",
            ));
        }
        return Ok(());
    }
    let selected_engine = match &record.selection {
        ActiveProfileReferenceState::Selected(reference) => reference.engine_id.clone(),
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
        } if proof_transition == transition_id && proof_engine == &selected_engine
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
    sync_publish_active_core_owner_proof(config_root, &proof)
}

pub(crate) async fn recover_old_owner_after_terminal(
    config_root: PathBuf,
    expected_revision: EngineSwitchRevision,
    terminal_generation: CoreGeneration,
) -> Result<Box<dyn EngineOwnerStoppedSwitchLease<ActiveProfileReferenceState>>, PortError> {
    let (config_root, _root_guard) = ActiveProfileReferenceFile::new(config_root)
        .acquire_root_lock()
        .await?;
    let _owner_state_guard = ActiveProfileReferenceFile::new(config_root.clone())
        .acquire_owner_state_lock()
        .await?;
    let mut record = sync_read_active_profile_record(&config_root)?;
    if record.selection.revision() != active_reference_revision(expected_revision) {
        return Err(PortError::Rejected(Failure::new(
            ErrorCode::NotReady,
            "active profile reference revision is stale",
            true,
        )));
    }
    let (
        transition_id,
        previous_reference,
        owner_identity,
        owner_generation,
        owner_token,
        persisted_terminal,
        selection_applied,
    ) = match &record.owner {
        ActiveCoreOwnerState::Switching {
            transition_id,
            previous_reference,
            owner_identity,
            owner_generation,
            owner_token,
            stage,
        } => {
            let (persisted_terminal, selection_applied) = match stage {
                ActiveOwnerSwitchStage::ClosingOld => (None, false),
                ActiveOwnerSwitchStage::OldStopped {
                    terminal_generation,
                } => (Some(*terminal_generation), false),
                ActiveOwnerSwitchStage::ReferenceChanged {
                    terminal_generation,
                } => (Some(*terminal_generation), true),
                ActiveOwnerSwitchStage::BindingNew { .. } => {
                    return Err(transition_rejected(
                        "old-owner terminal recovery cannot resume a candidate reservation",
                    ));
                }
            };
            (
                transition_id.clone(),
                previous_reference.clone(),
                owner_identity.clone(),
                *owner_generation,
                owner_token.clone(),
                persisted_terminal,
                selection_applied,
            )
        }
        ActiveCoreOwnerState::Unowned | ActiveCoreOwnerState::Bound { .. } => {
            return Err(transition_rejected(
                "old-owner terminal recovery requires the exact switch reservation",
            ));
        }
    };
    verify_terminal_generation(terminal_generation, owner_generation)?;
    if let Some(persisted_terminal) = persisted_terminal {
        if persisted_terminal != terminal_generation {
            return Err(transition_rejected(
                "old-owner terminal generation differs from the persisted stage",
            ));
        }
    }
    let closing_old = matches!(
        record.owner,
        ActiveCoreOwnerState::Switching {
            stage: ActiveOwnerSwitchStage::ClosingOld,
            ..
        }
    );
    if closing_old && record.selection != previous_reference {
        return Err(transition_rejected(
            "active reference changed before old-owner terminal recovery",
        ));
    }
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
    let persisted_stage = match (&record.owner, selection_applied) {
        (
            ActiveCoreOwnerState::Switching {
                stage: ActiveOwnerSwitchStage::ReferenceChanged { .. },
                ..
            },
            true,
        ) => ActiveOwnerSwitchStage::ReferenceChanged {
            terminal_generation,
        },
        _ => {
            let stage = ActiveOwnerSwitchStage::OldStopped {
                terminal_generation,
            };
            record.owner = ActiveCoreOwnerState::Switching {
                transition_id: transition_id.clone(),
                previous_reference: previous_reference.clone(),
                owner_identity: owner_identity.clone(),
                owner_generation,
                owner_token: owner_token.clone(),
                stage: stage.clone(),
            };
            sync_publish_active_profile_record(&config_root, &record)?;
            stage
        }
    };
    Ok(Box::new(ActiveOwnerStoppedSwitchLease::new(
        config_root,
        transition_id,
        owner_token,
        switch_stage(
            &persisted_stage,
            &previous_reference,
            &record.selection,
            &owner_identity,
            owner_generation,
        ),
    )))
}

pub(crate) async fn recover_candidate_after_terminal(
    config_root: PathBuf,
    expected_revision: EngineSwitchRevision,
    terminal_generation: CoreGeneration,
) -> Result<EngineSwitchSnapshot<ActiveProfileReferenceState>, PortError> {
    if terminal_generation != CoreGeneration::new(0) {
        return Err(transition_rejected(
            "candidate recovery requires a never-started terminal generation",
        ));
    }
    let (config_root, _root_guard) = ActiveProfileReferenceFile::new(config_root)
        .acquire_root_lock()
        .await?;
    let _owner_state_guard = ActiveProfileReferenceFile::new(config_root.clone())
        .acquire_owner_state_lock()
        .await?;
    let mut record = sync_read_active_profile_record(&config_root)?;
    if record.selection.revision() != active_reference_revision(expected_revision) {
        return Err(PortError::Rejected(Failure::new(
            ErrorCode::NotReady,
            "active profile reference revision is stale",
            true,
        )));
    }
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
                "no candidate owner reservation is pending",
            ));
        }
    };
    let live_local_candidate = local_owner_registration_matches(
        &config_root,
        &token,
        &LocalOwnerRegistrationState::Binding {
            transition_id: transition_id.clone(),
        },
    )?;
    if !live_local_candidate {
        if local_owner_registration_state(&config_root, &token)?.is_some() {
            return Err(transition_rejected(
                "candidate owner registration belongs to another switch",
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
        } if proof_identity.engine == engine_id
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
        identity: old_owner_identity.clone(),
        bound_generation: old_owner_generation,
        terminal_generation: old_terminal_generation,
        token: old_owner_token,
    };
    sync_publish_active_core_owner_proof(&config_root, &proof)?;
    record.owner = ActiveCoreOwnerState::Unowned;
    sync_publish_active_profile_record(&config_root, &record)?;
    if live_local_candidate {
        remove_local_owner_registration(&config_root, &token)?;
    }
    switch_snapshot(&record)
}
