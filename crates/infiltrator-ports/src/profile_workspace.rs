//! Coherent document/options reads and source-bound persistence proposals.
use infiltrator_contract::profile_protection::ProfileWriteProtection;
use infiltrator_contract::profile_source::ProfileSourceIdentity;
use infiltrator_domain::profile_options::ProfileOptions;

#[derive(Clone, Debug, PartialEq)]
pub struct ProfileWorkspace {
    pub source: ProfileSourceIdentity,
    pub write_protection: ProfileWriteProtection,
    pub content: String,
    pub options: ProfileOptions,
    /// Exact optional sidecar bytes, retained for source-bound rollback.
    pub options_document: Option<String>,
}

/// Derived provider/overlay changes remain allowed; direct editing requires an explicit unlock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileWorkspacePurpose {
    Derived,
    DirectEdit { allow_protected: bool },
}

#[derive(Clone, Debug)]
pub struct ProfileWorkspaceUpdate {
    pub purpose: ProfileWorkspacePurpose,
    pub content: String,
    pub options: ProfileOptions,
}
