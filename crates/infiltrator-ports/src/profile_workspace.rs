//! Coherent document/options reads and source-bound persistence proposals.
use infiltrator_contract::profile_document_identity::{
    ProfileDocument, ProfileSidecarRevision, ProfileWorkspaceIdentity,
};
use infiltrator_contract::profile_protection::{ProfileWriteAuthorization, ProfileWriteProtection};

#[derive(Clone, Debug, PartialEq)]
pub struct ProfileWorkspace {
    pub document: ProfileDocument,
    pub sidecar_revision: ProfileSidecarRevision,
    pub write_protection: ProfileWriteProtection,
    /// Opaque exact sidecar bytes, retained for source-bound rollback.
    pub sidecar_bytes: Option<Vec<u8>>,
}

impl ProfileWorkspace {
    pub fn identity(&self) -> ProfileWorkspaceIdentity {
        ProfileWorkspaceIdentity::new(
            self.document.identity.clone(),
            self.sidecar_revision.clone(),
        )
    }
}

/// Derived provider/overlay changes remain allowed; direct editing requires an explicit unlock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileWorkspacePurpose {
    Derived,
    DirectEdit {
        authorization: ProfileWriteAuthorization,
    },
}

#[derive(Clone, Debug)]
pub struct ProfileWorkspaceUpdate {
    pub purpose: ProfileWorkspacePurpose,
    pub document_bytes: Vec<u8>,
    /// Opaque exact sidecar bytes. Engine-specific applications own decoding.
    pub sidecar_bytes: Option<Vec<u8>>,
}
