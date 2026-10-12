//! Engine-scoped ProfileStore implementation over the codec-bound ConfigManager.

use crate::manager::ConfigManager;
use crate::manager::paths::profile_id;
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReference, ActiveProfileReferenceCasOutcome, ActiveProfileReferenceState,
    ActiveProfileReferenceTarget, ActiveProfileReferenceWriteAuthorization,
};
use infiltrator_contract::aggregator::AggregationTemplate;
use infiltrator_contract::apply_transaction::ApplyTransactionSnapshot;
use infiltrator_contract::core_identity::EngineId;
use infiltrator_contract::profile_document_identity::{ProfileId, ProfileWorkspaceIdentity};
use infiltrator_domain::profiles::{ProfileInfo, ProfileMetadata};
use infiltrator_ports::active_profile_reference_store::ActiveProfileReferenceStore;
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_store::{ActiveProfileWorkspaceWriteLease, ProfileStore};
use infiltrator_ports::profile_workspace::{ProfileWorkspace, ProfileWorkspaceUpdate};
use infiltrator_ports::secure_store::SecureStore;
use std::fmt::Display;
use std::path::PathBuf;

#[cfg(test)]
#[path = "profile_store_test.rs"]
mod profile_store_test;

#[async_trait::async_trait]
impl<S> ProfileStore for ConfigManager<S>
where
    S: SecureStore,
{
    fn engine_id(&self) -> EngineId {
        ConfigManager::engine_id(self)
    }

    fn config_dir(&self) -> PathBuf {
        ConfigManager::config_dir(self).to_path_buf()
    }

    fn apply_transaction(&self, profile: &str) -> Option<ApplyTransactionSnapshot> {
        ConfigManager::apply_transaction(self, profile)
    }

    async fn list_profiles(&self) -> Result<Vec<ProfileInfo>, PortError> {
        ConfigManager::list_profile_info(self).await
    }

    async fn get_current(&self) -> Result<String, PortError> {
        let lease = ActiveProfileReferenceStore::acquire_selection_lease(self).await?;
        self.profile_store
            .get_current_under_selection_lease(lease.as_ref())
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
            Err(PortError::Rejected(failure))
                if failure.code == infiltrator_contract::error::ErrorCode::NotReady =>
            {
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
        ensure_namespace(self).await?;
        let _guard = self.lock_profile_writes().await.map_err(storage_error)?;
        let profile = profile_id(profile).map_err(storage_error)?;
        self.profile_store.read_profile_document(&profile).await
    }

    async fn save_inactive_profile(&self, profile: &str, content: &str) -> Result<(), PortError> {
        ConfigManager::save_inactive_profile_port(self, profile, content).await
    }

    async fn delete_profile(&self, profile: &str) -> Result<(), PortError> {
        ConfigManager::delete_profile(self, profile)
            .await
            .map_err(storage_error)
    }

    async fn get_profile_metadata(&self, profile: &str) -> Result<ProfileMetadata, PortError> {
        ensure_namespace(self).await?;
        let _guard = self.lock_profile_writes().await.map_err(storage_error)?;
        let profile = profile_id(profile).map_err(storage_error)?;
        self.profile_store
            .get_profile_metadata(profile.as_str())
            .await
    }

    async fn update_profile_metadata(
        &self,
        profile: &str,
        metadata: &ProfileMetadata,
    ) -> Result<(), PortError> {
        ensure_namespace(self).await?;
        let _guard = crate::manager::lock_reset_fenced_profile_mutation_for_dir(
            ConfigManager::config_dir(self),
        )
        .await?;
        let profile = profile_id(profile).map_err(storage_error)?;
        self.profile_store
            .update_profile_metadata(profile.as_str(), metadata)
            .await
    }

    async fn load_workspace(&self, profile: &ProfileId) -> Result<ProfileWorkspace, PortError> {
        self.read_workspace(profile).await
    }
    async fn load_active_workspace(&self) -> Result<ProfileWorkspace, PortError> {
        self.read_active_workspace().await
    }

    async fn acquire_active_workspace_write_lease(
        &self,
        expected_reference: ActiveProfileReference,
        expected_workspace: ProfileWorkspaceIdentity,
        authorization: ActiveProfileReferenceWriteAuthorization,
    ) -> Result<Box<dyn ActiveProfileWorkspaceWriteLease + '_>, PortError> {
        ConfigManager::acquire_active_workspace_write_lease(
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
        ConfigManager::compare_and_save_unowned_active_derived_document(
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
        self.save_workspace(expected, update).await
    }

    async fn restore_inactive_workspace(
        &self,
        expected: &ProfileWorkspaceIdentity,
        previous: &ProfileWorkspace,
    ) -> Result<ProfileWorkspace, PortError> {
        self.recover_workspace(expected, previous).await
    }

    async fn delete_subscription_credential(&self, profile: &str) -> Result<(), PortError> {
        ConfigManager::delete_subscription_credential(self, profile)
            .await
            .map_err(storage_error)
    }

    async fn load_aggregation_templates(&self) -> Result<Vec<AggregationTemplate>, PortError> {
        ensure_namespace(self).await?;
        let _guard = self.lock_profile_writes().await.map_err(storage_error)?;
        self.profile_store.load_aggregation_templates_locked().await
    }

    async fn save_aggregation_templates(
        &self,
        templates: &[AggregationTemplate],
    ) -> Result<(), PortError> {
        ensure_namespace(self).await?;
        let _guard = crate::manager::lock_reset_fenced_profile_mutation_for_dir(
            ConfigManager::config_dir(self),
        )
        .await?;
        self.profile_store
            .save_aggregation_templates_locked(templates)
            .await
    }

    async fn clear_backup(&self, profile: &str) -> Result<(), PortError> {
        ConfigManager::clear_backup(self, profile)
            .await
            .map_err(storage_error)
    }

    async fn restore_inactive_profile_backup(&self, profile: &str) -> Result<bool, PortError> {
        ConfigManager::restore_inactive_profile_backup(self, profile)
            .await
            .map_err(storage_error)
    }
}

async fn ensure_namespace<S: SecureStore>(manager: &ConfigManager<S>) -> Result<(), PortError> {
    manager
        .initialize_profile_namespace()
        .await
        .map_err(storage_error)
}

fn storage_error<E: Display>(error: E) -> PortError {
    PortError::Io(error.to_string())
}
