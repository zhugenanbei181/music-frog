//! The exact documents a profile editor observed before proposing a change.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileSourceIdentity {
    pub profile: String,
    pub document_hash: String,
    /// None means the sidecar was absent; an existing empty file is distinct.
    pub options_hash: Option<String>,
}
