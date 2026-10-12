//! Engine-bound, codec-checked source persistence for profile documents.
use crate::manager::ConfigManager;
use infiltrator_contract::active_profile_reference::ActiveProfileReferenceState;
use infiltrator_contract::capability::Capability;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::profile_document_identity::{
    ProfileDocumentSourceIdentity, ProfileId, ProfileWorkspaceIdentity,
};
use infiltrator_contract::profile_protection::{ProfileWriteAuthorization, ProfileWriteProtection};
use infiltrator_domain::profile_options::options_path;
use infiltrator_domain::profiles::validate_profile_name;
use infiltrator_ports::active_profile_reference_store::{
    ActiveProfileReferenceLease, ActiveProfileReferenceStore,
};
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_workspace::{
    ProfileWorkspace, ProfileWorkspacePurpose, ProfileWorkspaceUpdate,
};
use infiltrator_ports::secure_store::SecureStore;
use infiltrator_profile_store::document_store::{
    read_optional_bytes, remove_optional, write_document_bytes_durable,
};
use infiltrator_profile_store::profile_files::profile_document_backup_path;
use infiltrator_profile_store::workspace::{
    invalid_workspace_recovery, publish_document_pair, selection_matches_profile, source_identity,
    validate_document_format, validate_engine, validate_workspace_bytes,
    validate_workspace_identity, workspace_from_bytes,
};
use mihomo_api::error::MihomoError;
use std::io::ErrorKind;
use tokio::fs;

#[derive(Clone, Copy)]
pub(crate) enum WorkspaceTarget {
    Inactive,
    AuthorizedActive,
}

enum SelectionFence {
    NotRequired,
    Held(Box<dyn ActiveProfileReferenceLease>),
}

impl<S: SecureStore> ConfigManager<S> {
    pub(crate) async fn read_active_workspace(&self) -> Result<ProfileWorkspace, PortError> {
        self.initialize_profile_namespace()
            .await
            .map_err(config_error)?;
        let lease = ActiveProfileReferenceStore::acquire_selection_lease(self).await?;
        let reference = match lease.state() {
            ActiveProfileReferenceState::Unselected { .. } => {
                return Err(PortError::unsupported(
                    Capability::Profiles,
                    "no product-wide active profile is selected",
                ));
            }
            ActiveProfileReferenceState::Selected(reference)
                if reference.engine_id == self.engine_id() =>
            {
                reference
            }
            ActiveProfileReferenceState::Selected(reference) => {
                return Err(PortError::unsupported(
                    Capability::Profiles,
                    format!(
                        "the active profile belongs to engine {} instead of {}",
                        reference.engine_id.as_str(),
                        self.engine_id().as_str()
                    ),
                ));
            }
        };
        let workspace = self.read_workspace(&reference.profile_id).await?;
        if workspace.document.identity.engine_id != reference.engine_id
            || workspace.document.identity.profile_id != reference.profile_id
        {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "active profile reference does not match the persisted profile document",
                true,
            )));
        }
        Ok(workspace)
    }

    pub(crate) async fn read_workspace(
        &self,
        profile: &ProfileId,
    ) -> Result<ProfileWorkspace, PortError> {
        self.initialize_profile_namespace()
            .await
            .map_err(config_error)?;
        let _guard = self.lock_profile_writes().await.map_err(config_error)?;
        self.read_workspace_locked(profile).await
    }

    pub(crate) async fn read_workspace_locked(
        &self,
        profile: &ProfileId,
    ) -> Result<ProfileWorkspace, PortError> {
        validate_profile_name(profile.as_str()).map_err(|error| {
            PortError::Rejected(Failure::new(
                ErrorCode::InvalidInput,
                error.to_string(),
                false,
            ))
        })?;
        let key = profile.as_str().to_owned();
        let profile_path = self
            .existing_profile_document_path(&key)
            .await
            .map_err(config_error)?;
        let document_bytes = fs::read(profile_path)
            .await
            .map_err(|error| config_error(MihomoError::Io(error)))?;
        let sidecar_path = options_path(self.config_dir(), &key);
        let sidecar_bytes = read_optional_bytes(&sidecar_path).await?;
        let protection = self.workspace_protection(&key).await?;
        let source = self.source_identity(profile, protection).await?;
        Ok(workspace_from_bytes(
            self.document_codec_scope(),
            profile.clone(),
            source,
            protection,
            document_bytes,
            sidecar_bytes,
        ))
    }

    pub(crate) async fn save_workspace(
        &self,
        expected: &ProfileWorkspaceIdentity,
        update: &ProfileWorkspaceUpdate,
        target: WorkspaceTarget,
    ) -> Result<ProfileWorkspace, PortError> {
        // Engine and format checks precede lock-key/path canonicalization and all file reads.
        validate_workspace_identity(self.document_codec_scope(), expected)?;
        self.validate_document_bytes(&update.document_bytes)?;
        self.publish_workspace(expected, update, target).await
    }

    pub(crate) async fn recover_workspace(
        &self,
        expected: &ProfileWorkspaceIdentity,
        previous: &ProfileWorkspace,
    ) -> Result<ProfileWorkspace, PortError> {
        // A foreign expected or recovery document must be rejected before lock/path/IO.
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
        self.validate_document_bytes(&previous.document.raw_bytes)?;
        self.publish_workspace(
            expected,
            &ProfileWorkspaceUpdate {
                purpose: ProfileWorkspacePurpose::Derived,
                document_bytes: previous.document.raw_bytes.clone(),
                sidecar_bytes: previous.sidecar_bytes.clone(),
            },
            WorkspaceTarget::Inactive,
        )
        .await
    }

    /// Restore exact prior bytes while the caller already holds the root
    /// reference and cross-process profile-write fences.
    pub(crate) async fn recover_workspace_locked(
        &self,
        expected: &ProfileWorkspaceIdentity,
        previous: &ProfileWorkspace,
    ) -> Result<ProfileWorkspace, PortError> {
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
        self.validate_document_bytes(&previous.document.raw_bytes)?;
        self.publish_workspace_locked(
            expected,
            &ProfileWorkspaceUpdate {
                purpose: ProfileWorkspacePurpose::Derived,
                document_bytes: previous.document.raw_bytes.clone(),
                sidecar_bytes: previous.sidecar_bytes.clone(),
            },
            WorkspaceTarget::AuthorizedActive,
            &SelectionFence::NotRequired,
        )
        .await
    }

    async fn publish_workspace(
        &self,
        expected: &ProfileWorkspaceIdentity,
        update: &ProfileWorkspaceUpdate,
        target: WorkspaceTarget,
    ) -> Result<ProfileWorkspace, PortError> {
        self.initialize_profile_namespace()
            .await
            .map_err(config_error)?;
        let selection_fence = match target {
            WorkspaceTarget::Inactive => SelectionFence::Held(
                ActiveProfileReferenceStore::acquire_selection_lease(self).await?,
            ),
            WorkspaceTarget::AuthorizedActive => SelectionFence::NotRequired,
        };
        let _guard = self.lock_profile_writes().await.map_err(config_error)?;
        self.publish_workspace_locked(expected, update, target, &selection_fence)
            .await
    }

    /// Publish while the caller already holds the root reference and
    /// cross-process profile-write fences. This is used by managed active
    /// content saves whose exact reference and owner authority were checked
    /// in the same transaction.
    pub(crate) async fn publish_active_workspace_locked(
        &self,
        expected: &ProfileWorkspaceIdentity,
        update: &ProfileWorkspaceUpdate,
    ) -> Result<ProfileWorkspace, PortError> {
        validate_workspace_identity(self.document_codec_scope(), expected)?;
        self.validate_document_bytes(&update.document_bytes)?;
        self.publish_workspace_locked(
            expected,
            update,
            WorkspaceTarget::AuthorizedActive,
            &SelectionFence::NotRequired,
        )
        .await
    }

    async fn publish_workspace_locked(
        &self,
        expected: &ProfileWorkspaceIdentity,
        update: &ProfileWorkspaceUpdate,
        target: WorkspaceTarget,
        selection_fence: &SelectionFence,
    ) -> Result<ProfileWorkspace, PortError> {
        let profile = &expected.document.profile_id;
        let workspace = self.read_workspace_locked(profile).await?;
        let target_matches = match target {
            WorkspaceTarget::Inactive => !match selection_fence {
                SelectionFence::NotRequired => false,
                SelectionFence::Held(lease) => {
                    selection_matches_profile(lease.state(), &self.engine_id(), profile.as_str())
                }
            },
            WorkspaceTarget::AuthorizedActive => true,
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

        // Pin both paths after the identity fence; the active-profile pointer cannot redirect a
        // prepared write. The exact prior sidecar bytes (including absence) are the rollback state.
        let profile_path = self
            .existing_profile_document_path(profile.as_str())
            .await
            .map_err(config_error)?;
        let sidecar_path = options_path(self.config_dir(), profile.as_str());
        let profile_publication =
            write_document_bytes_durable(&profile_path, &update.document_bytes);
        publish_document_pair(
            &sidecar_path,
            workspace.sidecar_bytes.as_deref(),
            update.sidecar_bytes.as_deref(),
            profile_publication,
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

    async fn source_identity(
        &self,
        profile: &ProfileId,
        protection: ProfileWriteProtection,
    ) -> Result<ProfileDocumentSourceIdentity, PortError> {
        let metadata = self
            .get_profile_metadata_unlocked(profile.as_str())
            .await
            .map_err(config_error)?;
        // The URL is never stored in the typed identity. If the secure store cannot return it,
        // the stable profile id remains a non-secret source fallback while protection stays on.
        source_identity(profile, protection, metadata.subscription_url.as_deref())
    }
}

pub(crate) fn config_error(error: MihomoError) -> PortError {
    match error {
        MihomoError::Io(error) if error.kind() == ErrorKind::PermissionDenied => {
            PortError::PermissionDenied(error.to_string())
        }
        MihomoError::Io(error) => PortError::Io(error.to_string()),
        MihomoError::NotFound(reason) => PortError::NotFound(reason),
        other => PortError::Rejected(Failure::new(
            ErrorCode::Configuration,
            other.to_string(),
            false,
        )),
    }
}

#[cfg(test)]
#[path = "profile_workspace_store_tests.rs"]
mod tests;
