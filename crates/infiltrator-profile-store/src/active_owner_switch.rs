//! Durable owner-switch lease transitions shared by all engine stores.
//!
//! The persisted record keeps the wire stages; the leases expose the neutral
//! [`EngineSwitchStage`] so the selector never orchestrates a second
//! vocabulary. [`stage_mapping`] is the only place that translates between the
//! two.

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
use crate::active_reference_owner_coordinator::ActiveProfileOwnerCoordinator;
use lurker_core_contract::active_profile_reference::{
    ActiveProfileReferenceRevision, ActiveProfileReferenceState,
};
use lurker_core_contract::engine_switch::{EngineSwitchSnapshot, EngineSwitchStage};
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::{CoreGeneration, CoreIdentity, EngineId};
use lurker_core_ports::engine_switch::{
    EngineOwnerBindingLease, EngineOwnerStoppedSwitchLease, EngineOwnerSwitchLease,
    EngineSwitchSelectionChangedLease,
};
use lurker_core_ports::error::PortError;
use lurker_profile_store::instance_lock::ThreadedInstanceLockGuard;
use std::path::{Path, PathBuf};

#[path = "active_owner_switch/stage_mapping.rs"]
pub(crate) mod stage_mapping;

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

/// Wire fields of the persisted switching reservation.
fn switching_state(
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

pub(crate) fn verify_terminal_generation(
    terminal_generation: CoreGeneration,
    owner_generation: CoreGeneration,
) -> Result<(), PortError> {
    if terminal_generation < owner_generation {
        return Err(transition_rejected(
            "old owner terminal generation predates its bound generation",
        ));
    }
    Ok(())
}

#[path = "active_owner_switch/switch_lease.rs"]
mod switch_lease;
use switch_lease::ActiveOwnerSwitchLease;

#[path = "active_owner_switch/stopped_lease.rs"]
mod stopped_lease;
use stopped_lease::ActiveOwnerStoppedSwitchLease;

#[path = "active_owner_switch/changed_lease.rs"]
mod changed_lease;
use changed_lease::ReferenceChangedLease;

#[path = "active_owner_switch/binding_lease.rs"]
mod binding_lease;
use binding_lease::OwnerBindingLease;

#[path = "active_owner_switch/recovery.rs"]
pub(crate) mod recovery;

impl ActiveProfileOwnerCoordinator {
    /// Begin a switch while the caller holds the root fence and has checked
    /// the product reset fence.
    pub async fn begin_owner_switch_while_root_locked(
        &self,
        root_guard: ThreadedInstanceLockGuard,
        expected_revision: ActiveProfileReferenceRevision,
        previous: ActiveProfileReferenceState,
        expected_owner: CoreIdentity,
        expected_binding_generation: CoreGeneration,
    ) -> Result<Box<dyn EngineOwnerSwitchLease<ActiveProfileReferenceState>>, PortError> {
        let _root_guard = root_guard;
        let config_root = self.config_root().to_path_buf();
        let mut record = sync_read_active_profile_record(&config_root)?;
        let _owner_state_guard = ActiveProfileReferenceFile::new(config_root.clone())
            .acquire_owner_state_lock()
            .await?;
        if record.selection.revision() != expected_revision || record.selection != previous {
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
        let stage = stage_mapping::switch_stage(
            &ActiveOwnerSwitchStage::ClosingOld,
            &previous_reference,
            &previous_reference,
            &owner_identity,
            owner_generation,
        );
        Ok(Box::new(ActiveOwnerSwitchLease::new(
            config_root,
            transition_id,
            owner_token,
            stage,
        )))
    }
}

#[cfg(test)]
#[path = "active_owner_switch_tests.rs"]
mod tests;
