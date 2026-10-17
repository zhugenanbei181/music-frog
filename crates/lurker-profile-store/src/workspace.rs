//! Engine-neutral identity, revision, and publication rules for profile
//! workspaces.
//!
//! A [`ProfileWorkspaceRecord`] is the kernel's single authority for one
//! coherent document/sidecar snapshot: its identity, exact bytes, sidecar
//! revision and write protection. Product crates project it into their own port
//! DTOs without adding business rules there.

use crate::document_store::{remove_optional, write_document_bytes_durable};
use async_trait::async_trait;
use lurker_core_contract::active_profile_reference::ActiveProfileReferenceState;
use lurker_core_contract::capability::Capability;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::EngineId;
use lurker_core_contract::profile_document_identity::{
    ProfileDocument, ProfileDocumentIdentity, ProfileDocumentRevision,
    ProfileDocumentSourceIdentity, ProfileDocumentSourceKind, ProfileFormatId, ProfileId,
    ProfileSchemaVersion, ProfileSidecarRevision, ProfileSourceId, ProfileWorkspaceIdentity,
};
use lurker_core_contract::profile_protection::ProfileWriteProtection;
use lurker_core_ports::error::PortError;
use lurker_core_ports::profile_document_codec::ProfileDocumentCodecScope;
use sha2::{Digest, Sha256};
use std::future::Future;
use std::path::Path;

/// One coherent document and sidecar snapshot owned by the kernel.
#[derive(Clone, Debug, PartialEq)]
pub struct ProfileWorkspaceRecord {
    pub document: ProfileDocument,
    pub sidecar_revision: ProfileSidecarRevision,
    pub write_protection: ProfileWriteProtection,
    /// Opaque exact sidecar bytes, retained for source-bound rollback.
    pub sidecar_bytes: Option<Vec<u8>>,
}

impl ProfileWorkspaceRecord {
    pub fn identity(&self) -> ProfileWorkspaceIdentity {
        ProfileWorkspaceIdentity::new(
            self.document.identity.clone(),
            self.sidecar_revision.clone(),
        )
    }
}

/// Product-owned facts that classify one profile's document source.
///
/// The kernel composes the source identity from these facts; it never imports a
/// credential store, a subscription catalog or a product settings schema.
#[async_trait]
pub trait ProfileSourceFacts: Send + Sync {
    /// Whether editing this profile's document requires the product's remote
    /// subscription rules instead of plain local editing.
    async fn write_protection(
        &self,
        profile: &ProfileId,
    ) -> Result<ProfileWriteProtection, PortError>;

    /// Subscription URL material. It is only hashed into a source identity and
    /// never persisted by the kernel.
    async fn subscription_url(&self, profile: &ProfileId) -> Result<Option<String>, PortError>;
}

/// Build the typed workspace identity from exact persisted document and sidecar
/// bytes.
pub fn workspace_from_bytes(
    codec_scope: &ProfileDocumentCodecScope,
    profile_id: ProfileId,
    source: ProfileDocumentSourceIdentity,
    write_protection: ProfileWriteProtection,
    document_bytes: Vec<u8>,
    sidecar_bytes: Option<Vec<u8>>,
) -> ProfileWorkspaceRecord {
    let identity = ProfileDocumentIdentity::new(
        profile_id,
        codec_scope.engine_id.clone(),
        source,
        document_revision(&document_bytes),
        codec_scope.format_id.clone(),
        codec_scope.schema_version,
    );
    let sidecar_revision = sidecar_revision(sidecar_bytes.as_deref());
    ProfileWorkspaceRecord {
        document: ProfileDocument::new(identity, document_bytes),
        sidecar_revision,
        write_protection,
        sidecar_bytes,
    }
}

/// Produce the next workspace projection while retaining source and protection
/// identity.
pub fn workspace_for_update(
    codec_scope: &ProfileDocumentCodecScope,
    previous: &ProfileWorkspaceRecord,
    document_bytes: Vec<u8>,
    sidecar_bytes: Option<Vec<u8>>,
) -> ProfileWorkspaceRecord {
    workspace_from_bytes(
        codec_scope,
        previous.document.identity.profile_id.clone(),
        previous.document.identity.source.clone(),
        previous.write_protection,
        document_bytes,
        sidecar_bytes,
    )
}

/// Reject a recovery projection whose revision does not match its retained raw
/// bytes.
pub fn validate_workspace_bytes(workspace: &ProfileWorkspaceRecord) -> Result<(), PortError> {
    if workspace.document.identity.revision != document_revision(&workspace.document.raw_bytes)
        || workspace.sidecar_revision != sidecar_revision(workspace.sidecar_bytes.as_deref())
    {
        return Err(invalid_workspace_recovery(
            "Recovery document bytes do not match their reported revision",
        ));
    }
    Ok(())
}

/// Validate both engine namespace and codec-authored format/schema before path
/// or I/O access.
pub fn validate_workspace_identity(
    codec_scope: &ProfileDocumentCodecScope,
    identity: &ProfileWorkspaceIdentity,
) -> Result<(), PortError> {
    validate_engine(&codec_scope.engine_id, &identity.document.engine_id)?;
    validate_document_format(
        codec_scope,
        &identity.document.format_id,
        identity.document.schema_version,
    )
}

/// Validate a document's adapter format and schema.
pub fn validate_document_format(
    codec_scope: &ProfileDocumentCodecScope,
    format_id: &ProfileFormatId,
    schema_version: ProfileSchemaVersion,
) -> Result<(), PortError> {
    if format_id != &codec_scope.format_id || schema_version != codec_scope.schema_version {
        return Err(PortError::unsupported(
            Capability::Profiles,
            format!(
                "profile codec for {} does not support format `{}` schema {}",
                codec_scope.engine_id.as_str(),
                format_id.as_str(),
                schema_version.value()
            ),
        ));
    }
    Ok(())
}

/// Reject identities outside the selected engine namespace.
pub fn validate_engine(expected: &EngineId, observed: &EngineId) -> Result<(), PortError> {
    if observed == expected {
        return Ok(());
    }
    Err(PortError::unsupported(
        Capability::Profiles,
        format!(
            "profile store for engine {} cannot access engine {}",
            expected.as_str(),
            observed.as_str()
        ),
    ))
}

/// Classify a profile source without persisting its potentially secret URL.
pub fn source_identity(
    profile: &ProfileId,
    protection: ProfileWriteProtection,
    subscription_url: Option<&str>,
) -> Result<ProfileDocumentSourceIdentity, PortError> {
    let source_kind = if protection.is_protected() {
        ProfileDocumentSourceKind::Subscription
    } else {
        ProfileDocumentSourceKind::Local
    };
    let source_material = subscription_url
        .filter(|url| !url.trim().is_empty())
        .map(str::as_bytes)
        .unwrap_or_else(|| profile.as_str().as_bytes());
    let source_id = ProfileSourceId::new(format!("source-{}", sha256_hex(source_material)))
        .map_err(|error| {
            PortError::Rejected(Failure::new(
                ErrorCode::InvalidInput,
                error.to_string(),
                false,
            ))
        })?;
    Ok(ProfileDocumentSourceIdentity::new(source_kind, source_id))
}

/// Check whether a revision lease selects the exact engine/profile pair.
pub fn selection_matches_profile(
    state: &ActiveProfileReferenceState,
    engine_id: &EngineId,
    profile_id: &str,
) -> bool {
    match state {
        ActiveProfileReferenceState::Unselected { .. } => false,
        ActiveProfileReferenceState::Selected(reference) => {
            reference.engine_id == *engine_id && reference.profile_id.as_str() == profile_id
        }
    }
}

/// Publish sidecar bytes and document bytes as one recoverable transaction.
pub async fn publish_document_pair(
    sidecar: &Path,
    previous: Option<&[u8]>,
    proposed: Option<&[u8]>,
    document_publication: impl Future<Output = Result<(), PortError>>,
) -> Result<(), PortError> {
    if previous == proposed {
        return document_publication.await;
    }
    publish_sidecar(sidecar, proposed).await?;
    if let Err(cause) = document_publication.await {
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

/// Calculate the exact content revision used by documents and sidecars.
pub fn document_revision(bytes: &[u8]) -> ProfileDocumentRevision {
    ProfileDocumentRevision::new(sha256_hex(bytes)).expect("SHA-256 revision is valid")
}

/// Preserve the distinction between an absent sidecar and an empty one.
pub fn sidecar_revision(bytes: Option<&[u8]>) -> ProfileSidecarRevision {
    match bytes {
        Some(bytes) => ProfileSidecarRevision::Present(document_revision(bytes)),
        None => ProfileSidecarRevision::Absent,
    }
}

/// Stable digest shared by source identities and recovery records.
pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

async fn publish_sidecar(path: &Path, bytes: Option<&[u8]>) -> Result<(), PortError> {
    match bytes {
        Some(bytes) => write_document_bytes_durable(path, bytes).await,
        None => remove_optional(path).await,
    }
}

pub fn invalid_workspace_recovery(message: &str) -> PortError {
    PortError::Rejected(Failure::new(ErrorCode::InvalidInput, message, false))
}

#[cfg(test)]
#[path = "workspace_tests.rs"]
mod tests;
