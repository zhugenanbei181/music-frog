//! Engine-owned profile document reads, backups, deletion and validated
//! publication.
//!
//! The kernel owns opaque document bytes, their exact previous copy and the
//! engine-scoped sidecar. Product-owned state that must be retired or removed
//! together with a profile is injected through
//! [`ProfileDeletionParticipant`]; the kernel never imports product code.

use crate::document_store::{remove_optional, write_document_bytes_durable};
use crate::store::ProfileStoreCore;
use crate::workspace::selection_matches_profile;
use async_trait::async_trait;
use lurker_core_contract::active_profile_reference::ActiveProfileReferenceState;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::profile_document::DocumentValidation;
use lurker_core_contract::profile_document_identity::ProfileId;
use lurker_core_ports::error::PortError;
use std::io::ErrorKind;
use tokio::fs;

/// Product-owned side effects of deleting one engine profile.
///
/// The kernel removes the document, its sidecar, its backup and the
/// engine-scoped metadata row. Credentials, subscription records and in-memory
/// observations belong to the product, so it supplies them here.
///
/// Ordering is part of the contract: `retire_for_deletion` runs *before* the
/// document disappears, and its guard must restore the exact previous product
/// state whenever deletion cannot complete.
#[async_trait]
pub trait ProfileDeletionParticipant: Send + Sync {
    /// Retire product-owned state bound to `profile`. The returned guard rolls
    /// the exact previous state back when the kernel aborts the deletion.
    async fn retire_for_deletion<'a>(
        &'a self,
        profile: &ProfileId,
    ) -> Result<Box<dyn ProfileDeletionRollback + 'a>, PortError>;

    /// Best-effort removal of product-owned records once the document and
    /// sidecar are gone. The kernel logs a failure here and still completes its
    /// own engine metadata cleanup.
    async fn complete_deletion(&self, profile: &ProfileId) -> Result<(), PortError>;
}

/// Restoration guard returned by
/// [`ProfileDeletionParticipant::retire_for_deletion`].
#[async_trait]
pub trait ProfileDeletionRollback: Send {
    async fn rollback(self: Box<Self>) -> Result<(), PortError>;
}

impl ProfileStoreCore {
    pub fn validate_document_bytes(&self, document: &[u8]) -> Result<(), PortError> {
        match self.document_codec().inspect(document)? {
            DocumentValidation::Valid => Ok(()),
            DocumentValidation::Invalid { diagnostics } => Err(PortError::Rejected(Failure::new(
                ErrorCode::Configuration,
                diagnostics
                    .first()
                    .map(|diagnostic| diagnostic.message.clone())
                    .unwrap_or_else(|| "engine profile codec rejected the document".into()),
                false,
            ))),
        }
    }

    pub async fn read_profile_document(&self, profile: &ProfileId) -> Result<String, PortError> {
        let path = self.existing_profile_document_path(profile).await?;
        fs::read_to_string(path)
            .await
            .map_err(|error| PortError::Io(error.to_string()))
    }

    pub async fn read_profile_document_backup(
        &self,
        profile: &ProfileId,
    ) -> Result<Option<String>, PortError> {
        let path = self.profile_document_path(profile).await?;
        match fs::read_to_string(self.layout().document_backup_path(&path)).await {
            Ok(document) => Ok(Some(document)),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
            Err(error) => Err(PortError::Io(error.to_string())),
        }
    }

    /// Publish a validated document and preserve the exact previous bytes.
    /// Callers must hold the selection fence and profile-write lock.
    pub async fn write_profile_document_locked(
        &self,
        profile: &ProfileId,
        document: &[u8],
    ) -> Result<(), PortError> {
        self.validate_document_bytes(document)?;
        self.write_profile_document_validated_locked(profile, document)
            .await
    }

    /// Save an inactive profile while the caller retains the product-wide
    /// selection fence. Validation and active-reference checks happen before
    /// the engine profile lock is acquired.
    pub async fn save_inactive_profile_under_selection_locked(
        &self,
        profile: &ProfileId,
        document: &[u8],
        selection: &ActiveProfileReferenceState,
    ) -> Result<(), PortError> {
        if selection_matches_profile(selection, &self.engine_id(), profile.as_str()) {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "Cannot write the active profile through an inactive-profile operation",
                false,
            )));
        }
        self.validate_document_bytes(document)?;
        let _profile_guard = self.lock_profile_writes().await?;
        self.write_profile_document_validated_locked(profile, document)
            .await
    }

    async fn write_profile_document_validated_locked(
        &self,
        profile: &ProfileId,
        document: &[u8],
    ) -> Result<(), PortError> {
        let path = self.profile_document_path(profile).await?;
        let backup = self.layout().document_backup_path(&path);
        if fs::try_exists(&path)
            .await
            .map_err(|error| PortError::Io(error.to_string()))?
        {
            let previous = fs::read(&path)
                .await
                .map_err(|error| PortError::Io(error.to_string()))?;
            write_document_bytes_durable(&backup, &previous).await?;
        } else {
            let _ = fs::remove_file(&backup).await;
        }
        write_document_bytes_durable(&path, document).await
    }

    /// Restore and validate the last saved document while caller-held fences
    /// prove that the profile is inactive.
    pub async fn restore_profile_document_backup_locked(
        &self,
        profile: &ProfileId,
    ) -> Result<bool, PortError> {
        let path = self.profile_document_path(profile).await?;
        let backup = self.layout().document_backup_path(&path);
        let previous = match fs::read(&backup).await {
            Ok(document) => document,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(PortError::Io(error.to_string())),
        };
        self.validate_document_bytes(&previous)?;
        write_document_bytes_durable(&path, &previous).await?;
        Ok(true)
    }

    pub async fn restore_inactive_profile_backup_under_selection_locked(
        &self,
        profile: &ProfileId,
        selection: &ActiveProfileReferenceState,
    ) -> Result<bool, PortError> {
        if selection_matches_profile(selection, &self.engine_id(), profile.as_str()) {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "Cannot restore an active profile backup through an inactive-profile operation",
                false,
            )));
        }
        let _profile_guard = self.lock_profile_writes().await?;
        self.restore_profile_document_backup_locked(profile).await
    }

    pub async fn clear_profile_document_backup(
        &self,
        profile: &ProfileId,
    ) -> Result<(), PortError> {
        let path = self.profile_document_path(profile).await?;
        remove_optional(&self.layout().document_backup_path(&path)).await
    }

    pub async fn profile_document_has_backup(
        &self,
        profile: &ProfileId,
    ) -> Result<bool, PortError> {
        let path = self.profile_document_path(profile).await?;
        fs::try_exists(self.layout().document_backup_path(&path))
            .await
            .map_err(|error| PortError::Io(error.to_string()))
    }

    /// Delete one inactive profile while the caller retains the exact
    /// product-wide selection lease. The engine write lock is acquired only
    /// after that root fence, preserving the shared root-to-profile lock order.
    pub async fn delete_profile_under_selection_locked(
        &self,
        profile: &ProfileId,
        selection: &ActiveProfileReferenceState,
        participant: &dyn ProfileDeletionParticipant,
    ) -> Result<(), PortError> {
        if selection_matches_profile(selection, &self.engine_id(), profile.as_str()) {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "Cannot delete the active profile",
                false,
            )));
        }
        let _profile_guard = self.lock_profile_writes().await?;
        self.delete_profile_locked(profile, participant).await
    }

    /// Remove one inactive profile's document and engine-owned sidecars while
    /// the caller holds the product selection fence and engine write lock.
    /// If sidecar removal fails, restore the original document and product
    /// state before returning the error.
    pub async fn delete_profile_locked(
        &self,
        profile: &ProfileId,
        participant: &dyn ProfileDeletionParticipant,
    ) -> Result<(), PortError> {
        let path = self.existing_profile_document_path(profile).await?;
        let previous_document = fs::read(&path)
            .await
            .map_err(|error| PortError::Io(error.to_string()))?;
        let sidecar_path = self
            .layout()
            .options_path(self.config_dir(), profile.as_str());
        let previous_sidecar = match fs::read(&sidecar_path).await {
            Ok(sidecar) => Some(sidecar),
            Err(error) if error.kind() == ErrorKind::NotFound => None,
            Err(error) => return Err(PortError::Io(error.to_string())),
        };
        let rollback = participant.retire_for_deletion(profile).await?;

        if let Err(error) = fs::remove_file(&path).await {
            if rollback.rollback().await.is_err() {
                return Err(PortError::Failed(
                    "profile deletion failed and product state rollback was not confirmed".into(),
                ));
            }
            return Err(PortError::Io(error.to_string()));
        }

        if let Err(error) = remove_optional(&sidecar_path).await {
            let document_restored = write_document_bytes_durable(&path, &previous_document).await;
            let sidecar_restored = match previous_sidecar.as_deref() {
                Some(sidecar) => write_document_bytes_durable(&sidecar_path, sidecar).await,
                None => Ok(()),
            };
            let product_restored = rollback.rollback().await;
            if document_restored.is_err() || sidecar_restored.is_err() || product_restored.is_err()
            {
                return Err(PortError::Failed(
                    "profile deletion failed and its rollback was not fully confirmed".into(),
                ));
            }
            return Err(error);
        }

        if let Err(error) = remove_optional(&self.layout().document_backup_path(&path)).await {
            log::warn!("deleted profile backup cleanup failed: {error}");
        }
        if let Err(error) = participant.complete_deletion(profile).await {
            log::warn!("deleted profile product cleanup failed: {error}");
        }
        self.remove_profile_metadata(profile.as_str()).await
    }
}

#[cfg(test)]
#[path = "profile_files_tests.rs"]
mod tests;
