//! Product facade over the neutral profile document/backup kernel.
//!
//! The kernel owns document bytes, their exact previous copy and the
//! engine-scoped sidecar. This module supplies the product-owned side effects of
//! deletion: the retiring Age credential of the profile and its subscription
//! record plus in-memory apply observation.

use crate::metadata::delete_subscription_url;
use crate::profile_age_credential::{
    ProfileAgeCredentialRetirement, retire_profile_age_credential_locked,
};
use crate::store::EngineProfileStore;
use infiltrator_ports::active_profile_reference_store::ActiveProfileReferenceLease;
use infiltrator_ports::secure_store::SecureStore;
use lurker_core_contract::profile_document_identity::ProfileId;
use lurker_core_ports::error::PortError;
use lurker_profile_store::profile_files::{ProfileDeletionParticipant, ProfileDeletionRollback};

impl<S: SecureStore> EngineProfileStore<S> {
    pub fn validate_document_bytes(&self, document: &[u8]) -> Result<(), PortError> {
        self.core().validate_document_bytes(document)
    }

    pub async fn read_profile_document(&self, profile: &ProfileId) -> Result<String, PortError> {
        self.core().read_profile_document(profile).await
    }

    pub async fn read_profile_document_backup(
        &self,
        profile: &ProfileId,
    ) -> Result<Option<String>, PortError> {
        self.core().read_profile_document_backup(profile).await
    }

    /// Publish a validated document and preserve the exact previous bytes.
    /// Callers must hold the selection fence and profile-write lock.
    pub async fn write_profile_document_locked(
        &self,
        profile: &ProfileId,
        document: &[u8],
    ) -> Result<(), PortError> {
        self.core()
            .write_profile_document_locked(profile, document)
            .await
    }

    /// Save an inactive profile while the caller retains the product-wide
    /// selection fence. Validation and active-reference checks happen before
    /// the engine profile lock is acquired.
    pub async fn save_inactive_profile_under_selection_lease(
        &self,
        profile: &ProfileId,
        document: &[u8],
        selection: &dyn ActiveProfileReferenceLease,
    ) -> Result<(), PortError> {
        self.core()
            .save_inactive_profile_under_selection_locked(profile, document, selection.state())
            .await
    }

    /// Restore and validate the last saved document while caller-held fences
    /// prove that the profile is inactive.
    pub async fn restore_profile_document_backup_locked(
        &self,
        profile: &ProfileId,
    ) -> Result<bool, PortError> {
        self.core()
            .restore_profile_document_backup_locked(profile)
            .await
    }

    pub async fn restore_inactive_profile_backup_under_selection_lease(
        &self,
        profile: &ProfileId,
        selection: &dyn ActiveProfileReferenceLease,
    ) -> Result<bool, PortError> {
        self.core()
            .restore_inactive_profile_backup_under_selection_locked(profile, selection.state())
            .await
    }

    pub async fn clear_profile_document_backup(
        &self,
        profile: &ProfileId,
    ) -> Result<(), PortError> {
        self.core().clear_profile_document_backup(profile).await
    }

    pub async fn profile_document_has_backup(
        &self,
        profile: &ProfileId,
    ) -> Result<bool, PortError> {
        self.core().profile_document_has_backup(profile).await
    }

    /// Delete one inactive profile while the caller retains the exact
    /// product-wide selection lease. The engine write lock is acquired only
    /// after that root fence, preserving the shared root-to-profile lock order.
    pub async fn delete_profile_under_selection_lease(
        &self,
        profile: &ProfileId,
        selection: &dyn ActiveProfileReferenceLease,
    ) -> Result<(), PortError> {
        self.core()
            .delete_profile_under_selection_locked(
                profile,
                selection.state(),
                &ProductDeletionParticipant { store: self },
            )
            .await
    }
}

/// Product-owned state retired and cleaned up together with one profile.
struct ProductDeletionParticipant<'a, S: SecureStore> {
    store: &'a EngineProfileStore<S>,
}

#[async_trait::async_trait]
impl<S: SecureStore> ProfileDeletionParticipant for ProductDeletionParticipant<'_, S> {
    async fn retire_for_deletion<'a>(
        &'a self,
        profile: &ProfileId,
    ) -> Result<Box<dyn ProfileDeletionRollback + 'a>, PortError> {
        let retirement = retire_profile_age_credential_locked(self.store, profile).await?;
        Ok(Box::new(ProductDeletionRollback {
            store: self.store,
            retirement,
        }))
    }

    async fn complete_deletion(&self, profile: &ProfileId) -> Result<(), PortError> {
        self.store.remove_apply_transaction(profile.as_str());
        if let Err(error) = delete_subscription_url(
            self.store.credential_store(),
            &self.store.engine_id(),
            profile.as_str(),
        )
        .await
        {
            log::warn!("failed to delete subscription entry: {error}");
        }
        Ok(())
    }
}

/// Restoration guard for the retired Age credential of one profile.
struct ProductDeletionRollback<'a, S: SecureStore> {
    store: &'a EngineProfileStore<S>,
    retirement: ProfileAgeCredentialRetirement,
}

#[async_trait::async_trait]
impl<S: SecureStore> ProfileDeletionRollback for ProductDeletionRollback<'_, S> {
    async fn rollback(self: Box<Self>) -> Result<(), PortError> {
        self.retirement.rollback(self.store).await
    }
}

#[cfg(test)]
#[path = "profile_files_tests.rs"]
mod tests;
