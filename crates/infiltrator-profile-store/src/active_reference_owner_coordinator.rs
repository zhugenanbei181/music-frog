//! Cross-process owner-proof normalization shared by all engine stores.

use crate::active_reference_file::ActiveProfileReferenceFile;
use crate::active_reference_owner::{
    ActiveCoreOwnerProof, ActiveCoreOwnerState, ActiveProfileReferenceRecord, transition_rejected,
};
use infiltrator_platform::instance_lock::ThreadedInstanceLockGuard;
use infiltrator_ports::error::PortError;
use std::path::PathBuf;

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

#[cfg(test)]
#[path = "active_reference_owner_coordinator_tests.rs"]
mod tests;
