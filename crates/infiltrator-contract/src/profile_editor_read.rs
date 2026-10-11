//! Independent document/options read status and source verification for native editors.
use crate::error::Failure;
use crate::profile_document::ProfileDocumentSnapshot;
use crate::profile_document_identity::ProfileWorkspaceIdentity;
use crate::profile_options::ProfileOptionsSnapshot;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProfileReadStatus {
    #[default]
    Unobserved,
    Loading,
    Ready,
    Failed(Failure),
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileEditorReadSnapshot {
    pub profile: Option<String>,
    pub observed_workspace_identity: Option<ProfileWorkspaceIdentity>,
    pub verified_workspace_identity: Option<ProfileWorkspaceIdentity>,
    pub document: ProfileReadStatus,
    pub options: ProfileReadStatus,
    pub verification_failure: Option<Failure>,
}
impl ProfileEditorReadSnapshot {
    pub fn source_current(&self) -> bool {
        self.verification_failure.is_none()
            && self.observed_workspace_identity.is_some()
            && self.observed_workspace_identity == self.verified_workspace_identity
    }
}

/// Editor observations remain available independently of the profile list page.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileEditorSnapshot {
    pub document: Option<ProfileDocumentSnapshot>,
    pub options: Option<ProfileOptionsSnapshot>,
    pub read: ProfileEditorReadSnapshot,
}
