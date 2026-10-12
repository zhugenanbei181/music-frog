//! Engine-scoped filesystem operations for coherent profile workspaces.

use crate::document_store::{read_optional_bytes, remove_optional, write_document_bytes_durable};
use crate::file_lock::lock_profile_writes_for_dir;
use crate::profile_files::profile_document_backup_path;
use crate::store::EngineProfileStore;
use crate::workspace::{
    invalid_workspace_recovery, publish_document_pair, selection_matches_profile, source_identity,
    validate_document_format, validate_engine, validate_workspace_bytes,
    validate_workspace_identity, workspace_from_bytes,
};
use infiltrator_contract::active_profile_reference::ActiveProfileReferenceState;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::profile_document_identity::{ProfileId, ProfileWorkspaceIdentity};
use infiltrator_contract::profile_protection::{ProfileWriteAuthorization, ProfileWriteProtection};
use infiltrator_domain::profile_options::options_path;
use infiltrator_domain::profiles::validate_profile_name;
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_workspace::{
    ProfileWorkspace, ProfileWorkspacePurpose, ProfileWorkspaceUpdate,
};
use infiltrator_ports::secure_store::SecureStore;
use tokio::fs;

/// Selection evidence held by the caller while a workspace publication runs.
/// `AuthorizedActive` is only used after the shared root/owner/profile fences
/// and typed owner authorization have been acquired by the coordinator.
pub enum ProfileWorkspacePublicationTarget<'a> {
    InactiveSelection(&'a ActiveProfileReferenceState),
    AuthorizedActive,
}

impl<S: SecureStore> EngineProfileStore<S> {
    /// Read one coherent document and sidecar while the caller holds the
    /// cross-process profile-write boundary.
    pub async fn read_profile_workspace_locked(
        &self,
        profile: &ProfileId,
    ) -> Result<ProfileWorkspace, PortError> {
        validate_profile_id(profile)?;
        let profile_path = self.existing_profile_document_path(profile).await?;
        let document_bytes = fs::read(profile_path).await.map_err(map_io_error)?;
        let sidecar_bytes =
            read_optional_bytes(&options_path(self.config_dir(), profile.as_str())).await?;
        let protection = self.workspace_write_protection(profile).await?;
        let metadata = self.get_profile_metadata(profile.as_str()).await?;
        let source = source_identity(profile, protection, metadata.subscription_url.as_deref())?;

        Ok(workspace_from_bytes(
            self.document_codec_scope(),
            profile.clone(),
            source,
            protection,
            document_bytes,
            sidecar_bytes,
        ))
    }

    /// Read a coherent workspace under the shared profile-write lock.
    pub async fn read_profile_workspace(
        &self,
        profile: &ProfileId,
    ) -> Result<ProfileWorkspace, PortError> {
        validate_profile_id(profile)?;
        let _guard = lock_profile_writes_for_dir(self.config_dir()).await?;
        self.read_profile_workspace_locked(profile).await
    }

    /// Validate recovery bytes before callers acquire filesystem locks or
    /// perform path access.
    pub fn validate_profile_workspace_recovery(
        &self,
        expected: &ProfileWorkspaceIdentity,
        previous: &ProfileWorkspace,
    ) -> Result<(), PortError> {
        validate_workspace_identity(self.document_codec_scope(), expected)?;
        validate_engine(&self.engine_id(), &previous.document.identity.engine_id)?;
        validate_document_format(
            self.document_codec_scope(),
            &previous.document.identity.format_id,
            previous.document.identity.schema_version,
        )?;
        validate_workspace_bytes(previous)?;
        if previous.document.identity.profile_id != expected.document.profile_id
            || previous.document.identity.source != expected.document.source
        {
            return Err(invalid_workspace_recovery(
                "Recovery document does not belong to the observed profile source",
            ));
        }
        self.validate_document_bytes(&previous.document.raw_bytes)
    }

    /// Publish exact document and sidecar bytes after the caller has acquired
    /// the selection, owner-state, and profile-write fences.
    pub async fn publish_profile_workspace_locked(
        &self,
        expected: &ProfileWorkspaceIdentity,
        update: &ProfileWorkspaceUpdate,
        target: ProfileWorkspacePublicationTarget<'_>,
    ) -> Result<ProfileWorkspace, PortError> {
        validate_workspace_identity(self.document_codec_scope(), expected)?;
        self.validate_document_bytes(&update.document_bytes)?;

        let profile = &expected.document.profile_id;
        let workspace = self.read_profile_workspace_locked(profile).await?;
        let target_matches = match target {
            ProfileWorkspacePublicationTarget::InactiveSelection(selection) => {
                !selection_matches_profile(selection, &self.engine_id(), profile.as_str())
            }
            ProfileWorkspacePublicationTarget::AuthorizedActive => true,
        };
        if workspace.identity() != *expected || !target_matches {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "The profile document or sidecar changed; reload and inspect before applying",
                true,
            )));
        }
        if matches!(
            update.purpose,
            ProfileWorkspacePurpose::DirectEdit {
                authorization: ProfileWriteAuthorization::RequireUnlock
            }
        ) && workspace.write_protection.is_protected()
        {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::Configuration,
                "Direct editing of this remote subscription requires an explicit unlock",
                false,
            )));
        }

        let profile_path = self.existing_profile_document_path(profile).await?;
        let sidecar_path = options_path(self.config_dir(), profile.as_str());
        let publication = write_document_bytes_durable(&profile_path, &update.document_bytes);
        publish_document_pair(
            &sidecar_path,
            workspace.sidecar_bytes.as_deref(),
            update.sidecar_bytes.as_deref(),
            publication,
        )
        .await?;
        if let Err(error) = remove_optional(&profile_document_backup_path(&profile_path)).await {
            log::warn!("committed workspace backup cleanup failed: {error}");
        }

        Ok(workspace_from_bytes(
            self.document_codec_scope(),
            profile.clone(),
            workspace.document.identity.source,
            workspace.write_protection,
            update.document_bytes.clone(),
            update.sidecar_bytes.clone(),
        ))
    }

    /// Restore exact previous bytes while the caller still holds all write
    /// fences. This retains the same source and revision compare rules as a
    /// normal workspace publication.
    pub async fn restore_profile_workspace_locked(
        &self,
        expected: &ProfileWorkspaceIdentity,
        previous: &ProfileWorkspace,
        target: ProfileWorkspacePublicationTarget<'_>,
    ) -> Result<ProfileWorkspace, PortError> {
        self.validate_profile_workspace_recovery(expected, previous)?;
        self.publish_profile_workspace_locked(
            expected,
            &ProfileWorkspaceUpdate {
                purpose: ProfileWorkspacePurpose::Derived,
                document_bytes: previous.document.raw_bytes.clone(),
                sidecar_bytes: previous.sidecar_bytes.clone(),
            },
            target,
        )
        .await
    }

    async fn workspace_write_protection(
        &self,
        profile: &ProfileId,
    ) -> Result<ProfileWriteProtection, PortError> {
        let settings = self.read_settings_value().await?;
        let Some(profiles) = settings.get("profiles") else {
            return Ok(ProfileWriteProtection::Editable);
        };
        let profiles = profiles.as_table().ok_or_else(invalid_metadata)?;
        let Some(metadata) = profiles.get(profile.as_str()) else {
            return Ok(ProfileWriteProtection::Editable);
        };
        let metadata = metadata.as_table().ok_or_else(invalid_metadata)?;
        for field in ["subscription_url", "subscription_url_key"] {
            if let Some(value) = metadata.get(field) {
                let value = value.as_str().ok_or_else(invalid_metadata)?;
                if !value.trim().is_empty() {
                    return Ok(ProfileWriteProtection::RemoteSubscription);
                }
            }
        }
        Ok(ProfileWriteProtection::Editable)
    }
}

fn validate_profile_id(profile: &ProfileId) -> Result<(), PortError> {
    validate_profile_name(profile.as_str()).map_err(|error| {
        PortError::Rejected(Failure::new(
            ErrorCode::InvalidInput,
            error.to_string(),
            false,
        ))
    })
}

fn invalid_metadata() -> PortError {
    PortError::Rejected(Failure::new(
        ErrorCode::Storage,
        "Invalid persisted profile ownership metadata",
        false,
    ))
}

fn map_io_error(error: std::io::Error) -> PortError {
    if error.kind() == std::io::ErrorKind::PermissionDenied {
        PortError::PermissionDenied(error.to_string())
    } else {
        PortError::Io(error.to_string())
    }
}

#[cfg(test)]
#[path = "workspace_store_tests.rs"]
mod tests;
