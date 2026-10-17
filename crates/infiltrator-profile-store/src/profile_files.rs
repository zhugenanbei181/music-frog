//! Engine-owned profile document reads, backups, and validated publication.

use crate::document_store::{remove_optional, write_document_bytes_durable};
use crate::file_lock::lock_profile_writes_for_dir;
use crate::metadata::delete_subscription_url;
use crate::profile_age_credential::retire_profile_age_credential_locked;
use crate::store::EngineProfileStore;
use infiltrator_contract::active_profile_reference::ActiveProfileReferenceState;
use infiltrator_contract::profile_document_identity::ProfileId;
use infiltrator_domain::profile_options::options_path;
use infiltrator_ports::active_profile_reference_store::ActiveProfileReferenceLease;
use infiltrator_ports::secure_store::SecureStore;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::profile_document::DocumentValidation;
use lurker_core_ports::error::PortError;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use tokio::fs;

impl<S: SecureStore> EngineProfileStore<S> {
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
        match fs::read_to_string(profile_document_backup_path(&path)).await {
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
    pub async fn save_inactive_profile_under_selection_lease(
        &self,
        profile: &ProfileId,
        document: &[u8],
        selection: &dyn ActiveProfileReferenceLease,
    ) -> Result<(), PortError> {
        if matches!(
            selection.state(),
            ActiveProfileReferenceState::Selected(reference)
                if reference.engine_id == self.engine_id()
                    && reference.profile_id == *profile
        ) {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "Cannot write the active profile through an inactive-profile operation",
                false,
            )));
        }
        self.validate_document_bytes(document)?;
        let _profile_guard = lock_profile_writes_for_dir(self.config_dir()).await?;
        self.write_profile_document_validated_locked(profile, document)
            .await
    }

    async fn write_profile_document_validated_locked(
        &self,
        profile: &ProfileId,
        document: &[u8],
    ) -> Result<(), PortError> {
        let path = self.profile_document_path(profile).await?;
        let backup = profile_document_backup_path(&path);
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
        let backup = profile_document_backup_path(&path);
        let previous = match fs::read(&backup).await {
            Ok(document) => document,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(PortError::Io(error.to_string())),
        };
        self.validate_document_bytes(&previous)?;
        write_document_bytes_durable(&path, &previous).await?;
        Ok(true)
    }

    pub async fn restore_inactive_profile_backup_under_selection_lease(
        &self,
        profile: &ProfileId,
        selection: &dyn ActiveProfileReferenceLease,
    ) -> Result<bool, PortError> {
        if matches!(
            selection.state(),
            ActiveProfileReferenceState::Selected(reference)
                if reference.engine_id == self.engine_id()
                    && reference.profile_id == *profile
        ) {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "Cannot restore an active profile backup through an inactive-profile operation",
                false,
            )));
        }
        let _profile_guard = lock_profile_writes_for_dir(self.config_dir()).await?;
        self.restore_profile_document_backup_locked(profile).await
    }

    pub async fn clear_profile_document_backup(
        &self,
        profile: &ProfileId,
    ) -> Result<(), PortError> {
        let path = self.profile_document_path(profile).await?;
        remove_optional(&profile_document_backup_path(&path)).await
    }

    pub async fn profile_document_has_backup(
        &self,
        profile: &ProfileId,
    ) -> Result<bool, PortError> {
        let path = self.profile_document_path(profile).await?;
        fs::try_exists(profile_document_backup_path(&path))
            .await
            .map_err(|error| PortError::Io(error.to_string()))
    }

    /// Delete one inactive profile while the caller retains the exact
    /// product-wide selection lease. The engine write lock is acquired only
    /// after that root fence, preserving the shared root-to-profile lock order.
    pub async fn delete_profile_under_selection_lease(
        &self,
        profile: &ProfileId,
        selection: &dyn ActiveProfileReferenceLease,
    ) -> Result<(), PortError> {
        if matches!(
            selection.state(),
            ActiveProfileReferenceState::Selected(reference)
                if reference.engine_id == self.engine_id()
                    && reference.profile_id == *profile
        ) {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "Cannot delete the active profile",
                false,
            )));
        }
        let _profile_guard = lock_profile_writes_for_dir(self.config_dir()).await?;
        self.delete_profile_locked(profile).await
    }

    /// Remove one inactive profile's document and engine-owned sidecars while
    /// the caller holds the product selection fence and engine write lock.
    /// If sidecar removal fails, restore the original document before
    /// returning the error.
    async fn delete_profile_locked(&self, profile: &ProfileId) -> Result<(), PortError> {
        let path = self.existing_profile_document_path(profile).await?;
        let previous_document = fs::read(&path)
            .await
            .map_err(|error| PortError::Io(error.to_string()))?;
        let sidecar_path = options_path(self.config_dir(), profile.as_str());
        let previous_sidecar = match fs::read(&sidecar_path).await {
            Ok(sidecar) => Some(sidecar),
            Err(error) if error.kind() == ErrorKind::NotFound => None,
            Err(error) => return Err(PortError::Io(error.to_string())),
        };
        let age_credential = retire_profile_age_credential_locked(self, profile).await?;

        if let Err(error) = fs::remove_file(&path).await {
            if age_credential.rollback(self).await.is_err() {
                return Err(PortError::Failed(
                    "profile deletion failed and Age credential rollback was not confirmed".into(),
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
            let credential_restored = age_credential.rollback(self).await;
            if document_restored.is_err()
                || sidecar_restored.is_err()
                || credential_restored.is_err()
            {
                return Err(PortError::Failed(
                    "profile deletion failed and its rollback was not fully confirmed".into(),
                ));
            }
            return Err(error);
        }

        if let Err(error) = remove_optional(&profile_document_backup_path(&path)).await {
            log::warn!("deleted profile backup cleanup failed: {error}");
        }
        self.remove_apply_transaction(profile.as_str());
        if let Err(error) =
            delete_subscription_url(self.credential_store(), &self.engine_id(), profile.as_str())
                .await
        {
            log::warn!("failed to delete subscription entry: {error}");
        }
        self.remove_profile_metadata(profile.as_str()).await
    }
}

pub fn profile_document_backup_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("profile.document");
    path.with_file_name(format!("{file_name}.bak"))
}

#[cfg(test)]
#[path = "profile_files_tests.rs"]
mod tests;
