//! Durable owner-switch lease transitions shared by all selection payloads.
//!
//! The persisted record keeps the wire stages; the leases expose the neutral
//! [`EngineSwitchStage`] so the selector never orchestrates a second
//! vocabulary. [`crate::owner_switch::stage`] is the only place that
//! translates between the two.

use crate::owner_switch::codec::OwnerSwitchCodec;
use crate::owner_switch::file::OwnerSwitchFile;
use crate::owner_switch::owner::{
    ActiveCoreOwnerLaunchProof, ActiveCoreOwnerProof, ActiveCoreOwnerProofRecord,
    ActiveCoreOwnerState, ActiveOwnerSwitchStage, ActiveOwnerToken, transition_rejected,
};
use lurker_core_contract::identity::{CoreGeneration, CoreIdentity};
use lurker_core_ports::error::PortError;
use std::path::Path;

pub mod binding_lease;
pub mod changed_lease;
pub mod stopped_lease;
pub mod switch_lease;

pub(crate) fn sync_read_active_profile_record<C: OwnerSwitchCodec>(
    config_root: &Path,
) -> Result<C::Record, PortError> {
    OwnerSwitchFile::<C>::new(config_root.to_path_buf())
        .read_active_profile_record_sync_or_pending()
}

pub(crate) fn sync_read_active_core_owner_proof<C: OwnerSwitchCodec>(
    config_root: &Path,
) -> Result<ActiveCoreOwnerProofRecord, PortError> {
    OwnerSwitchFile::<C>::new(config_root.to_path_buf()).read_owner_proof_sync()
}

pub(crate) fn sync_publish_active_core_owner_proof<C: OwnerSwitchCodec>(
    config_root: &Path,
    record: &ActiveCoreOwnerProofRecord,
) -> Result<(), PortError> {
    OwnerSwitchFile::<C>::new(config_root.to_path_buf()).publish_owner_proof_sync_reconciled(record)
}

pub(crate) fn sync_publish_active_profile_record<C: OwnerSwitchCodec>(
    config_root: &Path,
    record: &C::Record,
) -> Result<(), PortError> {
    OwnerSwitchFile::<C>::new(config_root.to_path_buf())
        .publish_active_profile_record_sync_reconciled(record)
}

/// Wire fields of the persisted switching reservation.
pub(crate) fn switching_state<C: OwnerSwitchCodec>(
    record: &C::Record,
    transition_id: &ActiveOwnerToken,
) -> Result<
    (
        C::Selection,
        CoreIdentity,
        CoreGeneration,
        ActiveOwnerToken,
        ActiveOwnerSwitchStage,
    ),
    PortError,
> {
    match C::owner(record) {
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
