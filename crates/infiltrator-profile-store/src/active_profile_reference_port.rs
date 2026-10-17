//! Product-root active-reference and owner-switch port for engine stores.

use crate::active_profile_selection_lease::ActiveProfileSelectionLease;
use crate::active_reference_cas::ActiveProfileReferenceTargetEvidence;
use crate::active_reference_file::ActiveProfileReferenceFile;
use crate::active_reference_owner::ActiveCoreOwnerState;
use crate::active_reference_owner_coordinator::ActiveProfileOwnerCoordinator;
use crate::reset_fence::{
    ActiveProfileResetMutationAccess, ensure_no_pending_reset_in_engine_namespaces,
    initialize_namespace_with_reset_fence,
};
use crate::store::EngineProfileStore;
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReferenceRevision, ActiveProfileReferenceState, ActiveProfileReferenceTarget,
    ActiveProfileReferenceWriteAuthorization,
};
use infiltrator_ports::active_profile_reference_store::{
    ActiveProfileOwnerRecoveryEvidence, ActiveProfileOwnerRecoveryOutcome,
    ActiveProfileOwnerStoppedSwitchLease, ActiveProfileOwnerSwitchLease,
    ActiveProfileOwnerSwitchRecovery, ActiveProfileReferenceLease, ActiveProfileReferenceStore,
};
use infiltrator_ports::secure_store::SecureStore;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::{CoreGeneration, CoreIdentity};
use lurker_core_contract::lifecycle::CoreLifecycleSnapshot;
use lurker_core_ports::error::PortError;

#[async_trait::async_trait]
impl<S: SecureStore> ActiveProfileReferenceStore for EngineProfileStore<S> {
    async fn read(&self) -> Result<ActiveProfileReferenceState, PortError> {
        Ok(
            ActiveProfileReferenceFile::new(self.config_root().to_path_buf())
                .read_active_profile_record_or_pending()
                .await?
                .selection,
        )
    }

    async fn acquire_selection_lease(
        &self,
    ) -> Result<Box<dyn ActiveProfileReferenceLease>, PortError> {
        initialize_namespace_with_reset_fence(self).await?;
        let (config_root, guard) =
            ActiveProfileReferenceFile::new(self.config_root().to_path_buf())
                .acquire_root_lock()
                .await?;
        ensure_no_pending_reset_in_engine_namespaces(
            &config_root,
            ActiveProfileResetMutationAccess::OrdinaryMutation,
        )
        .await?;
        let file = ActiveProfileReferenceFile::new(config_root.clone());
        let mut record = file.read_active_profile_record_or_pending().await?;
        let (_owner_state, _owner_state_guard) =
            ActiveProfileOwnerCoordinator::new(config_root.clone())
                .normalize_owner_state_unlocked(&mut record)
                .await?;
        Ok(Box::new(ActiveProfileSelectionLease::new(
            record.selection,
            config_root,
            guard,
        )))
    }

    async fn acquire_provider_selection_lease(
        &self,
    ) -> Result<Box<dyn ActiveProfileReferenceLease>, PortError> {
        initialize_namespace_with_reset_fence(self).await?;
        let (config_root, guard) =
            ActiveProfileReferenceFile::new(self.config_root().to_path_buf())
                .acquire_root_lock()
                .await?;
        ensure_no_pending_reset_in_engine_namespaces(
            &config_root,
            ActiveProfileResetMutationAccess::OrdinaryMutation,
        )
        .await?;
        let file = ActiveProfileReferenceFile::new(config_root.clone());
        let mut record = file.read_active_profile_record_or_pending().await?;
        let (owner_state, _owner_state_guard) =
            ActiveProfileOwnerCoordinator::new(config_root.clone())
                .normalize_owner_state_unlocked(&mut record)
                .await?;
        match owner_state {
            ActiveCoreOwnerState::Unowned => {}
            ActiveCoreOwnerState::Bound {
                identity,
                generation,
                ..
            } => {
                return Err(PortError::Rejected(Failure::new(
                    ErrorCode::NotReady,
                    format!(
                        "active Core {} at generation {} must close before another provider is composed",
                        identity.instance.as_str(),
                        generation.value()
                    ),
                    true,
                )));
            }
            ActiveCoreOwnerState::Switching { .. } => {
                return Err(PortError::Rejected(Failure::new(
                    ErrorCode::NotReady,
                    "an active provider switch must finish before another provider is composed",
                    true,
                )));
            }
        }
        Ok(Box::new(ActiveProfileSelectionLease::new(
            record.selection,
            config_root,
            guard,
        )))
    }

    async fn recover_bound_owner_after_terminal(
        &self,
        evidence: ActiveProfileOwnerRecoveryEvidence,
    ) -> Result<ActiveProfileOwnerRecoveryOutcome, PortError> {
        ActiveProfileOwnerCoordinator::new(self.config_root().to_path_buf())
            .recover_bound_owner_after_terminal(evidence)
            .await
    }

    async fn compare_and_set(
        &self,
        expected_revision: ActiveProfileReferenceRevision,
        target: ActiveProfileReferenceTarget,
        authorization: ActiveProfileReferenceWriteAuthorization,
    ) -> Result<ActiveProfileReferenceState, PortError> {
        initialize_namespace_with_reset_fence(self).await?;
        let (config_root, _guard) =
            ActiveProfileReferenceFile::new(self.config_root().to_path_buf())
                .acquire_root_lock()
                .await?;
        let mut record = ActiveProfileReferenceFile::new(config_root.clone())
            .read_active_profile_record_or_pending()
            .await?;
        ensure_no_pending_reset_in_engine_namespaces(
            &config_root,
            ActiveProfileResetMutationAccess::OrdinaryMutation,
        )
        .await?;
        ActiveProfileOwnerCoordinator::new(config_root)
            .compare_and_set_reference_unlocked(
                &mut record,
                expected_revision,
                target,
                authorization,
                self.engine_id(),
                |target, local_engine| async move {
                    match target {
                        ActiveProfileReferenceTarget::Unselected => {
                            Ok(ActiveProfileReferenceTargetEvidence::Unselected)
                        }
                        ActiveProfileReferenceTarget::Selected {
                            engine_id,
                            profile_id,
                        } if engine_id == local_engine => {
                            let workspace = self.read_profile_workspace(&profile_id).await?;
                            Ok(ActiveProfileReferenceTargetEvidence::LocalWorkspace(
                                workspace.identity(),
                            ))
                        }
                        ActiveProfileReferenceTarget::Selected { .. } => {
                            Ok(ActiveProfileReferenceTargetEvidence::OtherEngine)
                        }
                    }
                },
            )
            .await
    }

    async fn begin_owner_switch(
        &self,
        expected_revision: ActiveProfileReferenceRevision,
        expected_owner: CoreIdentity,
        expected_binding_generation: CoreGeneration,
    ) -> Result<Box<dyn ActiveProfileOwnerSwitchLease>, PortError> {
        let (config_root, root_guard) =
            ActiveProfileReferenceFile::new(self.config_root().to_path_buf())
                .acquire_root_lock()
                .await?;
        ensure_no_pending_reset_in_engine_namespaces(
            &config_root,
            ActiveProfileResetMutationAccess::OrdinaryMutation,
        )
        .await?;
        ActiveProfileOwnerCoordinator::new(config_root)
            .begin_owner_switch_while_root_locked(
                root_guard,
                expected_revision,
                expected_owner,
                expected_binding_generation,
            )
            .await
    }

    async fn recover_owner_switch(&self) -> Result<ActiveProfileOwnerSwitchRecovery, PortError> {
        ActiveProfileOwnerCoordinator::new(self.config_root().to_path_buf())
            .recover_owner_switch()
            .await
    }

    async fn recover_old_owner_after_terminal(
        &self,
        terminal: &CoreLifecycleSnapshot,
    ) -> Result<Box<dyn ActiveProfileOwnerStoppedSwitchLease>, PortError> {
        ActiveProfileOwnerCoordinator::new(self.config_root().to_path_buf())
            .recover_old_owner_after_terminal(terminal)
            .await
    }

    async fn recover_candidate_after_terminal(
        &self,
        terminal: &CoreLifecycleSnapshot,
    ) -> Result<ActiveProfileReferenceState, PortError> {
        ActiveProfileOwnerCoordinator::new(self.config_root().to_path_buf())
            .recover_candidate_after_terminal(terminal)
            .await
    }
}
