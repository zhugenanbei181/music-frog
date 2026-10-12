//! Engine-bound, source-checked persistence for Mihomo profile documents.
use crate::manager::ConfigManager;
use crate::manager::paths::sanitized_profile_key;
use crate::manager::profiles::backup_path;
use crate::profile_document_codec::{PROFILE_FORMAT, PROFILE_SCHEMA_VERSION};
use crate::sidecar_store::{read_optional_bytes, remove_optional, write_document_bytes_durable};
use crate::yaml::validate;
use infiltrator_contract::active_profile_reference::ActiveProfileReferenceState;
use infiltrator_contract::capability::Capability;
use infiltrator_contract::core_identity::EngineId;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::profile_document_identity::{
    ProfileDocument, ProfileDocumentIdentity, ProfileDocumentRevision,
    ProfileDocumentSourceIdentity, ProfileDocumentSourceKind, ProfileFormatId, ProfileId,
    ProfileSchemaVersion, ProfileSidecarRevision, ProfileSourceId, ProfileWorkspaceIdentity,
};
use infiltrator_contract::profile_protection::{ProfileWriteAuthorization, ProfileWriteProtection};
use infiltrator_domain::profile_options::options_path;
use infiltrator_ports::active_profile_reference_store::{
    ActiveProfileReferenceLease, ActiveProfileReferenceStore,
};
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_workspace::{
    ProfileWorkspace, ProfileWorkspacePurpose, ProfileWorkspaceUpdate,
};
use infiltrator_ports::secure_store::SecureStore;
use mihomo_api::error::MihomoError;
use sha2::{Digest, Sha256};
use std::fmt::Display;
use std::future::Future;
use std::io::ErrorKind;
use std::path::Path;
use std::str;
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
        let key = sanitized_profile_key(profile.as_str()).map_err(config_error)?;
        let profile_path = self
            .existing_profile_yaml_path(&key)
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
        ensure_mihomo_identity(expected)?;
        validate_document_format(
            &expected.document.format_id,
            expected.document.schema_version,
        )?;
        validate_document_bytes(&update.document_bytes)?;
        self.publish_workspace(expected, update, target).await
    }

    pub(crate) async fn recover_workspace(
        &self,
        expected: &ProfileWorkspaceIdentity,
        previous: &ProfileWorkspace,
    ) -> Result<ProfileWorkspace, PortError> {
        // A foreign expected or recovery document must be rejected before lock/path/IO.
        ensure_mihomo_identity(expected)?;
        ensure_mihomo_engine(&previous.document.identity.engine_id)?;
        validate_document_format(
            &previous.document.identity.format_id,
            previous.document.identity.schema_version,
        )?;
        validate_workspace_bytes(previous)?;
        if previous.document.identity.profile_id != expected.document.profile_id
            || previous.document.identity.source != expected.document.source
        {
            return Err(invalid_recovery(
                "Recovery document does not belong to the observed profile source",
            ));
        }
        validate_document_bytes(&previous.document.raw_bytes)?;
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
        ensure_mihomo_identity(expected)?;
        ensure_mihomo_engine(&previous.document.identity.engine_id)?;
        validate_document_format(
            &previous.document.identity.format_id,
            previous.document.identity.schema_version,
        )?;
        validate_workspace_bytes(previous)?;
        if previous.document.identity.profile_id != expected.document.profile_id
            || previous.document.identity.source != expected.document.source
        {
            return Err(invalid_recovery(
                "Recovery document does not belong to the observed profile source",
            ));
        }
        validate_document_bytes(&previous.document.raw_bytes)?;
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
        ensure_mihomo_identity(expected)?;
        validate_document_format(
            &expected.document.format_id,
            expected.document.schema_version,
        )?;
        validate_document_bytes(&update.document_bytes)?;
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
            WorkspaceTarget::Inactive => {
                !selection_matches_profile(selection_fence, &self.engine_id(), profile.as_str())
            }
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
            .existing_profile_yaml_path(profile.as_str())
            .await
            .map_err(config_error)?;
        let sidecar_path = options_path(self.config_dir(), profile.as_str());
        let profile_publication =
            write_document_bytes_durable(&profile_path, &update.document_bytes);
        publish_pair(
            &sidecar_path,
            workspace.sidecar_bytes.as_deref(),
            update.sidecar_bytes.as_deref(),
            profile_publication,
        )
        .await?;
        if let Err(error) = remove_optional(&backup_path(&profile_path)).await {
            log::warn!("committed workspace backup cleanup failed: {error}");
        }

        Ok(workspace_from_bytes(
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
        let source_kind = if protection.is_protected() {
            ProfileDocumentSourceKind::Subscription
        } else {
            ProfileDocumentSourceKind::Local
        };
        // The URL is never stored in the typed identity. If the secure store cannot return it,
        // the stable profile id remains a non-secret source fallback while protection stays on.
        let source_material = metadata
            .subscription_url
            .as_deref()
            .filter(|url| !url.trim().is_empty())
            .map(str::as_bytes)
            .unwrap_or_else(|| profile.as_str().as_bytes());
        let source_id = ProfileSourceId::new(format!("source-{}", sha256_hex(source_material)))
            .map_err(identity_error)?;
        Ok(ProfileDocumentSourceIdentity::new(source_kind, source_id))
    }
}

fn selection_matches_profile(
    fence: &SelectionFence,
    engine_id: &EngineId,
    profile_id: &str,
) -> bool {
    match fence {
        SelectionFence::NotRequired => false,
        SelectionFence::Held(lease) => match lease.state() {
            ActiveProfileReferenceState::Unselected { .. } => false,
            ActiveProfileReferenceState::Selected(reference) => {
                reference.engine_id == *engine_id && reference.profile_id.as_str() == profile_id
            }
        },
    }
}

fn workspace_from_bytes(
    profile_id: ProfileId,
    source: ProfileDocumentSourceIdentity,
    write_protection: ProfileWriteProtection,
    document_bytes: Vec<u8>,
    sidecar_bytes: Option<Vec<u8>>,
) -> ProfileWorkspace {
    let identity = ProfileDocumentIdentity::new(
        profile_id,
        crate::engine_id(),
        source,
        revision(&document_bytes),
        ProfileFormatId::new(PROFILE_FORMAT).expect("static profile format is valid"),
        ProfileSchemaVersion::new(PROFILE_SCHEMA_VERSION).expect("schema version is positive"),
    );
    let sidecar_revision = sidecar_revision(sidecar_bytes.as_deref());
    ProfileWorkspace {
        document: ProfileDocument::new(identity, document_bytes),
        sidecar_revision,
        write_protection,
        sidecar_bytes,
    }
}

pub(crate) fn workspace_for_update(
    previous: &ProfileWorkspace,
    update: &ProfileWorkspaceUpdate,
) -> ProfileWorkspace {
    workspace_from_bytes(
        previous.document.identity.profile_id.clone(),
        previous.document.identity.source.clone(),
        previous.write_protection.clone(),
        update.document_bytes.clone(),
        update.sidecar_bytes.clone(),
    )
}

fn validate_workspace_bytes(workspace: &ProfileWorkspace) -> Result<(), PortError> {
    if workspace.document.identity.revision != revision(&workspace.document.raw_bytes)
        || workspace.sidecar_revision != sidecar_revision(workspace.sidecar_bytes.as_deref())
    {
        return Err(invalid_recovery(
            "Recovery document bytes do not match their reported revision",
        ));
    }
    Ok(())
}

fn validate_document_format(
    format_id: &ProfileFormatId,
    schema_version: ProfileSchemaVersion,
) -> Result<(), PortError> {
    if format_id.as_str() != PROFILE_FORMAT || schema_version.value() != PROFILE_SCHEMA_VERSION {
        return Err(PortError::unsupported(
            Capability::Profiles,
            format!(
                "Mihomo profile adapter does not support format `{}` schema {}",
                format_id.as_str(),
                schema_version.value()
            ),
        ));
    }
    Ok(())
}

fn validate_document_bytes(document: &[u8]) -> Result<(), PortError> {
    let text = str::from_utf8(document).map_err(|error| {
        PortError::Rejected(Failure::new(
            ErrorCode::Configuration,
            format!("Mihomo YAML profile is not valid UTF-8: {error}"),
            false,
        ))
    })?;
    validate(text).map_err(|error| {
        PortError::Rejected(Failure::new(
            ErrorCode::Configuration,
            error.to_string(),
            false,
        ))
    })
}

fn ensure_mihomo_identity(identity: &ProfileWorkspaceIdentity) -> Result<(), PortError> {
    ensure_mihomo_engine(&identity.document.engine_id)?;
    validate_document_format(
        &identity.document.format_id,
        identity.document.schema_version,
    )
}

fn ensure_mihomo_engine(engine_id: &EngineId) -> Result<(), PortError> {
    if engine_id == &crate::engine_id() {
        return Ok(());
    }
    Err(PortError::unsupported(
        Capability::Profiles,
        format!(
            "Mihomo profile store cannot access engine `{}`",
            engine_id.as_str()
        ),
    ))
}

fn identity_error(error: impl Display) -> PortError {
    PortError::Rejected(Failure::new(
        ErrorCode::InvalidInput,
        error.to_string(),
        false,
    ))
}

fn invalid_recovery(message: &str) -> PortError {
    PortError::Rejected(Failure::new(ErrorCode::InvalidInput, message, false))
}

fn revision(bytes: &[u8]) -> ProfileDocumentRevision {
    ProfileDocumentRevision::new(sha256_hex(bytes)).expect("SHA-256 revision is valid")
}

fn sidecar_revision(bytes: Option<&[u8]>) -> ProfileSidecarRevision {
    match bytes {
        Some(bytes) => ProfileSidecarRevision::Present(revision(bytes)),
        None => ProfileSidecarRevision::Absent,
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Publish the opaque sidecar first and undo it if profile publication fails.
async fn publish_pair(
    sidecar: &Path,
    previous: Option<&[u8]>,
    proposed: Option<&[u8]>,
    profile_publication: impl Future<Output = Result<(), PortError>>,
) -> Result<(), PortError> {
    if previous == proposed {
        return profile_publication.await;
    }
    publish_sidecar(sidecar, proposed).await?;
    if let Err(cause) = profile_publication.await {
        if let Err(rollback) = publish_sidecar(sidecar, previous).await {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::InvalidState,
                format!(
                    "profile publication failed ({cause}); sidecar rollback failed ({rollback})"
                ),
                false,
            )));
        }
        return Err(cause);
    }
    Ok(())
}

async fn publish_sidecar(path: &Path, bytes: Option<&[u8]>) -> Result<(), PortError> {
    match bytes {
        Some(bytes) => write_document_bytes_durable(path, bytes).await,
        None => remove_optional(path).await,
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
