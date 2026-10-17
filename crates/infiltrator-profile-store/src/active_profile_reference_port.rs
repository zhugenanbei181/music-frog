//! Product-root active-reference and owner-switch journal for engine stores.

use crate::active_owner_lease::ActiveCoreOwnerLease;
use crate::active_owner_switch::stage_mapping::{active_reference_revision, switch_snapshot};
use crate::active_profile_selection_lease::ActiveProfileSelectionLease;
use crate::active_reference_cas::ActiveProfileReferenceTargetEvidence;
use crate::active_reference_file::ActiveProfileReferenceFile;
use crate::active_reference_owner::ProductCodec;
use crate::active_reference_owner_coordinator::ActiveProfileOwnerCoordinator;
use crate::reset_fence::{
    ensure_no_pending_reset_in_engine_namespaces, initialize_namespace_with_reset_fence,
};
use crate::store::EngineProfileStore;
use infiltrator_ports::active_profile_reference_store::{
    ActiveProfileOwnerLease, ActiveProfileReferenceLease, ActiveProfileReferenceStore,
};
use infiltrator_ports::secure_store::SecureStore;
use lurker_core_contract::active_profile_reference::{
    ActiveProfileReferenceRevision, ActiveProfileReferenceState, ActiveProfileReferenceTarget,
    ActiveProfileReferenceWriteAuthorization,
};
use lurker_core_contract::capability::Capability;
use lurker_core_contract::engine_switch::{
    EngineSwitchRecovery, EngineSwitchRevision, EngineSwitchSnapshot,
};
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::{CoreGeneration, CoreIdentity};
use lurker_core_ports::engine_switch::{
    EngineOwnerLease, EngineOwnerStoppedSwitchLease, EngineOwnerSwitchLease, EngineSwitchJournal,
    OwnerRecoveryEvidence, OwnerRecoveryOutcome,
};
use lurker_core_ports::error::PortError;
use lurker_profile_store::owner_switch::owner::{ActiveCoreOwnerState, transition_rejected};
use lurker_profile_store::owner_switch::recovery::{
    recover_candidate_after_terminal, recover_old_owner_after_terminal, recover_owner_switch,
};
use lurker_profile_store::owner_switch::registry::{
    LocalOwnerRegistrationState, local_owner_registration_matches,
};
use lurker_profile_store::owner_switch::reset::ActiveProfileResetMutationAccess;

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

    async fn adopt_finalized_owner(
        &self,
        identity: CoreIdentity,
        generation: CoreGeneration,
    ) -> Result<Box<dyn ActiveProfileOwnerLease>, PortError> {
        let config_root = self.config_root().to_path_buf();
        let record = ActiveProfileReferenceFile::new(config_root.clone())
            .read_active_profile_record_or_pending()
            .await?;
        let token = match &record.owner {
            ActiveCoreOwnerState::Bound {
                identity: bound_identity,
                generation: bound_generation,
                token,
            } if bound_identity.same_instance(&identity) && *bound_generation == generation => {
                token.clone()
            }
            _ => {
                return Err(transition_rejected(
                    "the finalized owner binding is not the persisted bound owner",
                ));
            }
        };
        if !local_owner_registration_matches(
            &config_root,
            &token,
            &LocalOwnerRegistrationState::Bound {
                identity: identity.clone(),
                generation,
            },
        )? {
            return Err(transition_rejected(
                "this process does not hold the finalized owner kernel registration",
            ));
        }
        Ok(Box::new(ActiveCoreOwnerLease::new(
            identity,
            generation,
            token,
            config_root,
        )))
    }

    async fn recover_bound_owner_after_terminal(
        &self,
        evidence: OwnerRecoveryEvidence,
    ) -> Result<OwnerRecoveryOutcome<ActiveProfileReferenceState>, PortError> {
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
}

#[async_trait::async_trait]
impl<S: SecureStore> EngineSwitchJournal for EngineProfileStore<S> {
    type Selection = ActiveProfileReferenceState;

    async fn snapshot(
        &self,
    ) -> Result<EngineSwitchSnapshot<ActiveProfileReferenceState>, PortError> {
        let record = ActiveProfileReferenceFile::new(self.config_root().to_path_buf())
            .read_active_profile_record_or_pending()
            .await?;
        switch_snapshot(&record)
    }

    /// Direct journal binding bypasses the product's selection lease, so it is
    /// not an available operation for this store. The product always binds the
    /// first owner through `ActiveProfileReferenceLease::into_owner_binding`.
    async fn bind_owner(
        &self,
        _expected_revision: EngineSwitchRevision,
        _selection: ActiveProfileReferenceState,
        _identity: CoreIdentity,
        _generation: CoreGeneration,
    ) -> Result<
        (
            EngineSwitchSnapshot<ActiveProfileReferenceState>,
            Box<dyn EngineOwnerLease>,
        ),
        PortError,
    > {
        Err(PortError::unsupported(
            Capability::Profiles,
            "direct engine-switch owner binding is unavailable; bind through the active-profile selection lease",
        ))
    }

    async fn begin_owner_switch(
        &self,
        expected_revision: EngineSwitchRevision,
        previous: ActiveProfileReferenceState,
        owner_identity: CoreIdentity,
        owner_generation: CoreGeneration,
    ) -> Result<Box<dyn EngineOwnerSwitchLease<ActiveProfileReferenceState>>, PortError> {
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
                active_reference_revision(expected_revision),
                previous,
                owner_identity,
                owner_generation,
            )
            .await
    }

    async fn recover_owner_switch(
        &self,
        expected_revision: EngineSwitchRevision,
    ) -> Result<EngineSwitchRecovery<ActiveProfileReferenceState>, PortError> {
        recover_owner_switch::<ProductCodec>(self.config_root().to_path_buf(), expected_revision)
            .await
    }

    async fn recover_old_owner_after_terminal(
        &self,
        expected_revision: EngineSwitchRevision,
        terminal_generation: CoreGeneration,
    ) -> Result<Box<dyn EngineOwnerStoppedSwitchLease<ActiveProfileReferenceState>>, PortError>
    {
        recover_old_owner_after_terminal::<ProductCodec>(
            self.config_root().to_path_buf(),
            expected_revision,
            terminal_generation,
        )
        .await
    }

    async fn recover_candidate_after_terminal(
        &self,
        expected_revision: EngineSwitchRevision,
        terminal_generation: CoreGeneration,
    ) -> Result<EngineSwitchSnapshot<ActiveProfileReferenceState>, PortError> {
        recover_candidate_after_terminal::<ProductCodec>(
            self.config_root().to_path_buf(),
            expected_revision,
            terminal_generation,
        )
        .await
    }
}
