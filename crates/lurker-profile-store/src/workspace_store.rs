//! Engine-scoped filesystem operations for coherent profile workspaces.
//!
//! The kernel validates and publishes exact document/sidecar bytes; products
//! supply the profile source facts through [`ProfileSourceFacts`], and their own
//! port DTO projection stays outside this crate.

use crate::document_path::validate_document_profile_id;
use crate::document_store::{
    read_optional_bytes_with_platform, read_required_bytes_with_platform,
    remove_optional_with_platform, write_document_bytes_durable_with_platform,
};
use crate::store::ProfileStoreCore;
use crate::workspace::{
    ProfileSourceFacts, ProfileWorkspaceRecord, invalid_workspace_recovery,
    publish_document_pair_with_platform, selection_matches_profile, source_identity,
    validate_document_format, validate_engine, validate_workspace_bytes,
    validate_workspace_identity, workspace_from_bytes,
};
use lurker_core_contract::active_profile_reference::ActiveProfileReferenceState;
use lurker_core_contract::capability::Capability;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::profile_document_identity::{ProfileId, ProfileWorkspaceIdentity};
use lurker_core_contract::profile_protection::ProfileWriteAuthorization;
use lurker_core_ports::error::PortError;

/// Selection evidence held by the caller while a workspace publication runs.
/// `AuthorizedActive` is only used after the shared root/owner/profile fences
/// and typed owner authorization have been acquired by the product coordinator.
pub enum ProfileWorkspacePublicationTarget<'a> {
    InactiveSelection(&'a ActiveProfileReferenceState),
    AuthorizedActive,
}

/// Why a workspace publication happens. Derived updates are always allowed;
/// direct editing of a protected document needs an explicit unlock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileWorkspaceEdit {
    Derived,
    DirectEdit {
        authorization: ProfileWriteAuthorization,
    },
}

impl ProfileStoreCore {
    /// Resolve the product-selected workspace under a retained root selection
    /// lease, then acquire the engine profile lock in root-to-profile order.
    pub async fn read_active_workspace_under_selection(
        &self,
        selection: &ActiveProfileReferenceState,
        facts: &dyn ProfileSourceFacts,
    ) -> Result<ProfileWorkspaceRecord, PortError> {
        let reference = match selection {
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
        let workspace = self
            .read_profile_workspace(&reference.profile_id, facts)
            .await?;
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

    /// Read one coherent document and sidecar while the caller holds the
    /// cross-process profile-write boundary.
    pub async fn read_profile_workspace_locked(
        &self,
        profile: &ProfileId,
        facts: &dyn ProfileSourceFacts,
    ) -> Result<ProfileWorkspaceRecord, PortError> {
        validate_document_profile_id(profile)?;
        let profile_path = self.existing_profile_document_path(profile).await?;
        let document_bytes =
            read_required_bytes_with_platform(self.platform(), &profile_path).await?;
        let sidecar_bytes = read_optional_bytes_with_platform(
            self.platform(),
            &self
                .layout()
                .options_path(self.config_dir(), profile.as_str()),
        )
        .await?;
        let protection = facts.write_protection(profile).await?;
        let source = source_identity(
            profile,
            protection,
            facts.subscription_url(profile).await?.as_deref(),
        )?;

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
        facts: &dyn ProfileSourceFacts,
    ) -> Result<ProfileWorkspaceRecord, PortError> {
        validate_document_profile_id(profile)?;
        let _guard = self.lock_profile_writes().await?;
        self.read_profile_workspace_locked(profile, facts).await
    }

    /// Validate recovery bytes before callers acquire filesystem locks or
    /// perform path access.
    pub fn validate_profile_workspace_recovery(
        &self,
        expected: &ProfileWorkspaceIdentity,
        previous: &ProfileWorkspaceRecord,
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
        facts: &dyn ProfileSourceFacts,
        expected: &ProfileWorkspaceIdentity,
        edit: ProfileWorkspaceEdit,
        document_bytes: &[u8],
        sidecar_bytes: Option<&[u8]>,
        target: ProfileWorkspacePublicationTarget<'_>,
    ) -> Result<ProfileWorkspaceRecord, PortError> {
        validate_workspace_identity(self.document_codec_scope(), expected)?;
        self.validate_document_bytes(document_bytes)?;

        let profile = &expected.document.profile_id;
        let workspace = self.read_profile_workspace_locked(profile, facts).await?;
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
            edit,
            ProfileWorkspaceEdit::DirectEdit {
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
        let sidecar_path = self
            .layout()
            .options_path(self.config_dir(), profile.as_str());
        let publication = write_document_bytes_durable_with_platform(
            self.platform(),
            &profile_path,
            document_bytes,
        );
        publish_document_pair_with_platform(
            self.platform(),
            &sidecar_path,
            workspace.sidecar_bytes.as_deref(),
            sidecar_bytes,
            publication,
        )
        .await?;
        if let Err(error) = remove_optional_with_platform(
            self.platform(),
            &self.layout().document_backup_path(&profile_path),
        )
        .await
        {
            log::warn!("committed workspace backup cleanup failed: {error}");
        }

        Ok(workspace_from_bytes(
            self.document_codec_scope(),
            profile.clone(),
            workspace.document.identity.source,
            workspace.write_protection,
            document_bytes.to_vec(),
            sidecar_bytes.map(<[u8]>::to_vec),
        ))
    }

    /// Restore exact previous bytes while the caller still holds all write
    /// fences. This retains the same source and revision compare rules as a
    /// normal workspace publication.
    pub async fn restore_profile_workspace_locked(
        &self,
        facts: &dyn ProfileSourceFacts,
        expected: &ProfileWorkspaceIdentity,
        previous: &ProfileWorkspaceRecord,
        target: ProfileWorkspacePublicationTarget<'_>,
    ) -> Result<ProfileWorkspaceRecord, PortError> {
        self.validate_profile_workspace_recovery(expected, previous)?;
        self.publish_profile_workspace_locked(
            facts,
            expected,
            ProfileWorkspaceEdit::Derived,
            &previous.document.raw_bytes,
            previous.sidecar_bytes.as_deref(),
            target,
        )
        .await
    }
}

#[cfg(test)]
#[path = "workspace_store_tests.rs"]
mod tests;
