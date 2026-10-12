//! ConfigManager coordinates namespace and lock lifecycle for shared workspace IO.

use crate::manager::ConfigManager;
use infiltrator_contract::active_profile_reference::ActiveProfileReferenceState;
use infiltrator_contract::capability::Capability;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::profile_document_identity::{ProfileId, ProfileWorkspaceIdentity};
use infiltrator_ports::active_profile_reference_store::ActiveProfileReferenceStore;
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_workspace::{ProfileWorkspace, ProfileWorkspaceUpdate};
use infiltrator_ports::secure_store::SecureStore;
use infiltrator_profile_store::workspace_store::ProfileWorkspacePublicationTarget;
use mihomo_api::error::MihomoError;
use std::io::ErrorKind;

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
        self.profile_store
            .read_profile_workspace_locked(profile)
            .await
    }

    pub(crate) async fn save_workspace(
        &self,
        expected: &ProfileWorkspaceIdentity,
        update: &ProfileWorkspaceUpdate,
    ) -> Result<ProfileWorkspace, PortError> {
        infiltrator_profile_store::workspace::validate_workspace_identity(
            self.document_codec_scope(),
            expected,
        )?;
        self.profile_store
            .validate_document_bytes(&update.document_bytes)?;
        self.publish_workspace(expected, update).await
    }

    pub(crate) async fn recover_workspace(
        &self,
        expected: &ProfileWorkspaceIdentity,
        previous: &ProfileWorkspace,
    ) -> Result<ProfileWorkspace, PortError> {
        self.profile_store
            .validate_profile_workspace_recovery(expected, previous)?;
        self.publish_workspace(
            expected,
            &ProfileWorkspaceUpdate {
                purpose: infiltrator_ports::profile_workspace::ProfileWorkspacePurpose::Derived,
                document_bytes: previous.document.raw_bytes.clone(),
                sidecar_bytes: previous.sidecar_bytes.clone(),
            },
        )
        .await
    }

    pub(crate) async fn recover_workspace_locked(
        &self,
        expected: &ProfileWorkspaceIdentity,
        previous: &ProfileWorkspace,
    ) -> Result<ProfileWorkspace, PortError> {
        self.profile_store
            .restore_profile_workspace_locked(
                expected,
                previous,
                ProfileWorkspacePublicationTarget::AuthorizedActive,
            )
            .await
    }

    async fn publish_workspace(
        &self,
        expected: &ProfileWorkspaceIdentity,
        update: &ProfileWorkspaceUpdate,
    ) -> Result<ProfileWorkspace, PortError> {
        self.initialize_profile_namespace()
            .await
            .map_err(config_error)?;
        let selection_lease = ActiveProfileReferenceStore::acquire_selection_lease(self).await?;
        let _guard = self.lock_profile_writes().await.map_err(config_error)?;
        self.profile_store
            .publish_profile_workspace_locked(
                expected,
                update,
                ProfileWorkspacePublicationTarget::InactiveSelection(selection_lease.state()),
            )
            .await
    }

    /// Publish while the caller already holds root, owner-state, and profile
    /// write fences for this exact active workspace.
    pub(crate) async fn publish_active_workspace_locked(
        &self,
        expected: &ProfileWorkspaceIdentity,
        update: &ProfileWorkspaceUpdate,
    ) -> Result<ProfileWorkspace, PortError> {
        self.profile_store
            .publish_profile_workspace_locked(
                expected,
                update,
                ProfileWorkspacePublicationTarget::AuthorizedActive,
            )
            .await
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
