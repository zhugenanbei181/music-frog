//! Independent document/options read status and source verification for native editors.
use crate::error::Failure;
use crate::profile_document::ProfileDocumentSnapshot;
use crate::profile_options::ProfileOptionsSnapshot;
use crate::profile_source::ProfileSourceIdentity;
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
    pub observed_source: Option<ProfileSourceIdentity>,
    pub verified_source: Option<ProfileSourceIdentity>,
    pub document: ProfileReadStatus,
    pub options: ProfileReadStatus,
    pub verification_failure: Option<Failure>,
}
impl ProfileEditorReadSnapshot {
    pub fn source_current(&self) -> bool {
        self.verification_failure.is_none()
            && self.observed_source.is_some()
            && self.observed_source == self.verified_source
    }
}

/// Editor observations remain available independently of the profile list page.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileEditorSnapshot {
    pub document: Option<ProfileDocumentSnapshot>,
    pub options: Option<ProfileOptionsSnapshot>,
    pub read: ProfileEditorReadSnapshot,
}
