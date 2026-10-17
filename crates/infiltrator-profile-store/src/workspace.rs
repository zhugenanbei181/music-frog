//! Product projection of the neutral profile workspace record.
//!
//! The kernel owns the single coherent document/sidecar snapshot and every
//! publication rule. Products only map [`ProfileWorkspaceRecord`] into the port
//! DTO this workspace has historically exposed.

use infiltrator_ports::profile_workspace::{ProfileWorkspace, ProfileWorkspaceUpdate};
use lurker_core_contract::profile_document_identity::ProfileDocumentRevision;
use lurker_core_ports::error::PortError;
use lurker_core_ports::profile_document_codec::ProfileDocumentCodecScope;
use lurker_profile_store::workspace;
use lurker_profile_store::workspace::ProfileWorkspaceRecord;
use std::future::Future;
use std::path::Path;

/// Project a port workspace into the neutral kernel record.
pub(crate) fn workspace_record(workspace: &ProfileWorkspace) -> ProfileWorkspaceRecord {
    ProfileWorkspaceRecord {
        document: workspace.document.clone(),
        sidecar_revision: workspace.sidecar_revision.clone(),
        write_protection: workspace.write_protection,
        sidecar_bytes: workspace.sidecar_bytes.clone(),
    }
}

/// Project a neutral kernel record into the port workspace.
pub(crate) fn product_workspace(record: ProfileWorkspaceRecord) -> ProfileWorkspace {
    ProfileWorkspace {
        document: record.document,
        sidecar_revision: record.sidecar_revision,
        write_protection: record.write_protection,
        sidecar_bytes: record.sidecar_bytes,
    }
}

/// Produce the next workspace projection while retaining source and protection
/// identity.
pub fn workspace_for_update(
    codec_scope: &ProfileDocumentCodecScope,
    previous: &ProfileWorkspace,
    update: &ProfileWorkspaceUpdate,
) -> ProfileWorkspace {
    product_workspace(workspace::workspace_for_update(
        codec_scope,
        &workspace_record(previous),
        update.document_bytes.clone(),
        update.sidecar_bytes.clone(),
    ))
}

/// Calculate the exact content revision used by documents and sidecars.
pub fn document_revision(bytes: &[u8]) -> ProfileDocumentRevision {
    workspace::document_revision(bytes)
}

/// Stable digest shared by source identities and recovery records.
pub fn sha256_hex(bytes: &[u8]) -> String {
    workspace::sha256_hex(bytes)
}

/// Publish sidecar bytes and document bytes as one recoverable transaction.
pub async fn publish_document_pair(
    sidecar: &Path,
    previous: Option<&[u8]>,
    proposed: Option<&[u8]>,
    document_publication: impl Future<Output = Result<(), PortError>>,
) -> Result<(), PortError> {
    workspace::publish_document_pair(sidecar, previous, proposed, document_publication).await
}
