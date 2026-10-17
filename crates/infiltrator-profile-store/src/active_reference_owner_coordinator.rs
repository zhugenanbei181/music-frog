//! Cross-process Core owner normalization and terminal recovery shared by all
//! engine stores.
//!
//! Normalization, terminal recovery and the switch reservation are the neutral
//! [`lurker_profile_store::owner_switch::coordinator`] machinery bound to the
//! product record codec. Only the product reference CAS authorization and
//! target-document evidence stay here.

use crate::active_reference_cas::{
    ActiveProfileReferenceCasReady, ActiveProfileReferenceTargetEvidence, owner_fence,
};
use crate::active_reference_file::ActiveProfileReferenceFile;
use crate::active_reference_owner::{
    ActiveProfileReferenceRecord, LegacySelectionMigration, ProductCodec,
};
use lurker_core_contract::active_profile_reference::{
    ActiveProfileReferenceRevision, ActiveProfileReferenceState, ActiveProfileReferenceTarget,
    ActiveProfileReferenceWriteAuthorization,
};
use lurker_core_contract::engine_switch::EngineSwitchRevision;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::{CoreGeneration, CoreIdentity, EngineId};
use lurker_core_ports::engine_switch::{
    EngineOwnerSwitchLease, OwnerRecoveryEvidence, OwnerRecoveryOutcome,
};
use lurker_core_ports::error::PortError;
use lurker_profile_store::instance_lock::ThreadedInstanceLockGuard;
use lurker_profile_store::owner_switch::coordinator::OwnerSwitchCoordinator;
use lurker_profile_store::owner_switch::owner::{ActiveCoreOwnerState, transition_rejected};
use lurker_profile_store::owner_switch::registry::{
    LocalOwnerRegistrationState, local_owner_registration_matches,
};
use std::future::Future;
use std::path::{Path, PathBuf};

/// Product-root owner-state reconciliation. The caller must hold the root
/// reference lock before normalizing; this coordinator acquires owner-state
/// after it, preserving the root -> owner-state lock order.
pub struct ActiveProfileOwnerCoordinator {
    inner: OwnerSwitchCoordinator<ProductCodec>,
}

impl ActiveProfileOwnerCoordinator {
    pub fn new(config_root: impl Into<PathBuf>) -> Self {
        Self {
            inner: OwnerSwitchCoordinator::<ProductCodec>::new(config_root),
        }
    }

    pub(crate) fn config_root(&self) -> &Path {
        self.inner.config_root()
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
        validate_reference_write_authorization(&owner_state, &authorization, self.config_root())?;
        let target_evidence = load_target_evidence(target.clone(), local_engine.clone()).await?;
        validate_target_evidence(&target, &local_engine, target_evidence)?;

        record.selection = cas_ready.transition(target, owner_fence(&owner_state))?;
        record.legacy_migration = LegacySelectionMigration::ExplicitSelection;
        ActiveProfileReferenceFile::new(self.config_root().to_path_buf())
            .publish_active_profile_record_sync_reconciled(record)?;
        Ok(record.selection.clone())
    }

    /// Reconcile a persisted owner only after the host has supplied exact
    /// process-exit evidence and a matching terminal lifecycle snapshot.
    pub async fn recover_bound_owner_after_terminal(
        &self,
        evidence: OwnerRecoveryEvidence,
    ) -> Result<OwnerRecoveryOutcome<ActiveProfileReferenceState>, PortError> {
        self.inner
            .recover_bound_owner_after_terminal(evidence)
            .await
    }

    pub async fn normalize_owner_state_unlocked(
        &self,
        record: &mut ActiveProfileReferenceRecord,
    ) -> Result<
        (
            ActiveCoreOwnerState<ActiveProfileReferenceState>,
            ThreadedInstanceLockGuard,
        ),
        PortError,
    > {
        self.inner.normalize_owner_state_unlocked(record).await
    }

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
        self.inner
            .begin_owner_switch_while_root_locked(
                root_guard,
                EngineSwitchRevision::new(expected_revision.value()),
                previous,
                expected_owner,
                expected_binding_generation,
            )
            .await
    }
}

pub fn validate_reference_write_authorization(
    owner_state: &ActiveCoreOwnerState<ActiveProfileReferenceState>,
    authorization: &ActiveProfileReferenceWriteAuthorization,
    config_root: &Path,
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

/// Keeps the neutral owner-fence vocabulary name available to product callers.
#[cfg(test)]
#[path = "active_reference_owner_coordinator_tests.rs"]
mod tests;
