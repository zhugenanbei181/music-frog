//! Opaque source identity binds a simulation and subsequent rule edits to one document.
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleSourceIdentity {
    pub profile: String,
    pub document_hash: String,
}
