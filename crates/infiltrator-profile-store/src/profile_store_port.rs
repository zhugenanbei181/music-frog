//! Production `ProfileStore` port over the engine-neutral persistence owner.

use crate::file_lock::lock_profile_writes_for_dir;
use crate::metadata::delete_subscription_url;
use crate::reset_fence::{
    acquire_reset_fenced_profile_mutation_for_dir, initialize_namespace_with_reset_fence,
};
use crate::store::EngineProfileStore;
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReference, ActiveProfileReferenceCasOutcome, ActiveProfileReferenceState,
    ActiveProfileReferenceTarget, ActiveProfileReferenceWriteAuthorization,
};
use infiltrator_contract::aggregator::AggregationTemplate;
use infiltrator_contract::apply_transaction::ApplyTransactionSnapshot;
use infiltrator_contract::core_identity::EngineId;
use infiltrator_contract::error::ErrorCode;
use infiltrator_contract::profile_catalog_order::{
    ProfileCatalogOrderMutation, ProfileCatalogOrderRequest,
};
use infiltrator_contract::profile_display_name::{
    ProfileDisplayNameMutation, ProfileDisplayNameObservation, ProfileDisplayNameTarget,
};
use infiltrator_contract::profile_document_identity::{ProfileId, ProfileWorkspaceIdentity};
use infiltrator_domain::profiles::{ProfileInfo, ProfileMetadata};
use infiltrator_ports::active_profile_reference_store::ActiveProfileReferenceStore;
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_store::{ActiveProfileWorkspaceWriteLease, ProfileStore};
use infiltrator_ports::profile_workspace::{ProfileWorkspace, ProfileWorkspaceUpdate};
use infiltrator_ports::secure_store::SecureStore;
use std::path::PathBuf;

#[async_trait::async_trait]
impl<S: SecureStore> ProfileStore for EngineProfileStore<S> {
    fn engine_id(&self) -> EngineId {
        EngineProfileStore::engine_id(self)
    }

    fn config_dir(&self) -> PathBuf {
        EngineProfileStore::config_dir(self).to_path_buf()
    }

    fn apply_transaction(&self, profile: &str) -> Option<ApplyTransactionSnapshot> {
        EngineProfileStore::apply_transaction(self, profile)
    }

    async fn list_profiles(&self) -> Result<Vec<ProfileInfo>, PortError> {
        initialize_namespace_with_reset_fence(self).await?;
        if !self.config_dir().exists() {
            return Ok(Vec::new());
        }
        let selection = ActiveProfileReferenceStore::acquire_selection_lease(self).await?;
        let _profile_guard = lock_profile_writes_for_dir(self.config_dir()).await?;
        self.list_profile_info_locked(selection.state()).await
    }

    async fn set_profile_catalog_order(
        &self,
        request: ProfileCatalogOrderRequest,
    ) -> Result<ProfileCatalogOrderMutation, PortError> {
        EngineProfileStore::set_profile_catalog_order(self, request).await
    }

    async fn get_current(&self) -> Result<String, PortError> {
        let selection = ActiveProfileReferenceStore::acquire_selection_lease(self).await?;
        self.get_current_under_selection_lease(selection.as_ref())
    }

    async fn read_active_profile_reference(
        &self,
    ) -> Result<ActiveProfileReferenceState, PortError> {
        ActiveProfileReferenceStore::read(self).await
    }

    async fn compare_and_set_active_profile_reference(
        &self,
        expected: ActiveProfileReferenceState,
        target: ActiveProfileReferenceTarget,
        authorization: ActiveProfileReferenceWriteAuthorization,
    ) -> Result<ActiveProfileReferenceCasOutcome, PortError> {
        let expected_revision = expected.revision();
        match ActiveProfileReferenceStore::compare_and_set(
            self,
            expected_revision,
            target,
            authorization,
        )
        .await
        {
            Ok(current) => Ok(ActiveProfileReferenceCasOutcome::Committed {
                previous: expected,
                current,
            }),
            Err(PortError::Rejected(failure)) if failure.code == ErrorCode::NotReady => {
                let observed = ActiveProfileReferenceStore::read(self).await?;
                if observed.revision() == expected_revision {
                    Err(PortError::Rejected(failure))
                } else {
                    Ok(ActiveProfileReferenceCasOutcome::Stale { expected, observed })
                }
            }
            Err(error) => Err(error),
        }
    }

    async fn load(&self, profile: &str) -> Result<String, PortError> {
        initialize_namespace_with_reset_fence(self).await?;
        let _profile_guard = lock_profile_writes_for_dir(self.config_dir()).await?;
        let profile = parse_profile_id(profile)?;
        self.read_profile_document(&profile).await
    }

    async fn save_inactive_profile(&self, profile: &str, content: &str) -> Result<(), PortError> {
        initialize_namespace_with_reset_fence(self).await?;
        let selection = ActiveProfileReferenceStore::acquire_selection_lease(self).await?;
        let profile = parse_profile_id(profile)?;
        self.save_inactive_profile_under_selection_lease(
            &profile,
            content.as_bytes(),
            selection.as_ref(),
        )
        .await
    }

    async fn delete_profile(&self, profile: &str) -> Result<(), PortError> {
        initialize_namespace_with_reset_fence(self).await?;
        let selection = ActiveProfileReferenceStore::acquire_selection_lease(self).await?;
        let profile = parse_profile_id(profile)?;
        self.delete_profile_under_selection_lease(&profile, selection.as_ref())
            .await
    }

    async fn get_profile_metadata(&self, profile: &str) -> Result<ProfileMetadata, PortError> {
        initialize_namespace_with_reset_fence(self).await?;
        let _profile_guard = lock_profile_writes_for_dir(self.config_dir()).await?;
        let profile = parse_profile_id(profile)?;
        EngineProfileStore::get_profile_metadata(self, profile.as_str()).await
    }

    async fn update_profile_metadata(
        &self,
        profile: &str,
        metadata: &ProfileMetadata,
    ) -> Result<(), PortError> {
        initialize_namespace_with_reset_fence(self).await?;
        let _guard = acquire_reset_fenced_profile_mutation_for_dir(self.config_dir()).await?;
        let profile = parse_profile_id(profile)?;
        EngineProfileStore::update_profile_metadata(self, profile.as_str(), metadata).await
    }

    async fn profile_display_name(
        &self,
        profile: &ProfileId,
    ) -> Result<ProfileDisplayNameObservation, PortError> {
        EngineProfileStore::profile_display_name(self, profile).await
    }

    async fn set_profile_display_name(
        &self,
        profile: &ProfileId,
        target: ProfileDisplayNameTarget,
    ) -> Result<ProfileDisplayNameMutation, PortError> {
        EngineProfileStore::set_profile_display_name(self, profile, target).await
    }

    async fn delete_subscription_credential(&self, profile: &str) -> Result<(), PortError> {
        initialize_namespace_with_reset_fence(self).await?;
        let _guard = acquire_reset_fenced_profile_mutation_for_dir(self.config_dir()).await?;
        let profile = parse_profile_id(profile)?;
        delete_subscription_url(self.credential_store(), &self.engine_id(), profile.as_str()).await
    }

    async fn load_workspace(&self, profile: &ProfileId) -> Result<ProfileWorkspace, PortError> {
        initialize_namespace_with_reset_fence(self).await?;
        self.read_profile_workspace(profile).await
    }

    async fn load_active_workspace(&self) -> Result<ProfileWorkspace, PortError> {
        initialize_namespace_with_reset_fence(self).await?;
        let selection = ActiveProfileReferenceStore::acquire_selection_lease(self).await?;
        self.read_active_workspace_under_selection_lease(selection.as_ref())
            .await
    }

    async fn acquire_active_workspace_write_lease(
        &self,
        expected_reference: ActiveProfileReference,
        expected_workspace: ProfileWorkspaceIdentity,
        authorization: ActiveProfileReferenceWriteAuthorization,
    ) -> Result<Box<dyn ActiveProfileWorkspaceWriteLease + '_>, PortError> {
        EngineProfileStore::acquire_active_workspace_write_lease(
            self,
            expected_reference,
            expected_workspace,
            authorization,
        )
        .await
    }

    async fn compare_and_save_unowned_active_derived_document(
        &self,
        expected_reference: ActiveProfileReference,
        expected_workspace: ProfileWorkspaceIdentity,
        update: &ProfileWorkspaceUpdate,
    ) -> Result<ProfileWorkspace, PortError> {
        EngineProfileStore::compare_and_save_unowned_active_derived_document(
            self,
            expected_reference,
            expected_workspace,
            update,
        )
        .await
    }

    async fn compare_and_save_inactive_workspace(
        &self,
        expected: &ProfileWorkspaceIdentity,
        update: &ProfileWorkspaceUpdate,
    ) -> Result<ProfileWorkspace, PortError> {
        EngineProfileStore::compare_and_save_inactive_workspace(self, expected, update).await
    }

    async fn restore_inactive_workspace(
        &self,
        expected: &ProfileWorkspaceIdentity,
        previous: &ProfileWorkspace,
    ) -> Result<ProfileWorkspace, PortError> {
        EngineProfileStore::restore_inactive_workspace(self, expected, previous).await
    }

    async fn load_aggregation_templates(&self) -> Result<Vec<AggregationTemplate>, PortError> {
        initialize_namespace_with_reset_fence(self).await?;
        let _profile_guard = lock_profile_writes_for_dir(self.config_dir()).await?;
        self.load_aggregation_templates_locked().await
    }

    async fn save_aggregation_templates(
        &self,
        templates: &[AggregationTemplate],
    ) -> Result<(), PortError> {
        initialize_namespace_with_reset_fence(self).await?;
        let _guard = acquire_reset_fenced_profile_mutation_for_dir(self.config_dir()).await?;
        self.save_aggregation_templates_locked(templates).await
    }

    async fn clear_backup(&self, profile: &str) -> Result<(), PortError> {
        initialize_namespace_with_reset_fence(self).await?;
        let _guard = acquire_reset_fenced_profile_mutation_for_dir(self.config_dir()).await?;
        let profile = parse_profile_id(profile)?;
        self.clear_profile_document_backup(&profile).await
    }

    async fn restore_inactive_profile_backup(&self, profile: &str) -> Result<bool, PortError> {
        initialize_namespace_with_reset_fence(self).await?;
        let selection = ActiveProfileReferenceStore::acquire_selection_lease(self).await?;
        let profile = parse_profile_id(profile)?;
        self.restore_inactive_profile_backup_under_selection_lease(&profile, selection.as_ref())
            .await
    }
}

fn parse_profile_id(profile: &str) -> Result<ProfileId, PortError> {
    ProfileId::new(profile.to_owned()).map_err(|error| PortError::Io(error.to_string()))
}
