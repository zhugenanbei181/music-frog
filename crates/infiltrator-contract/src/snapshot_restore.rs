//! Frozen snapshot restoration reviews and actual source-bound terminal receipts.
use crate::error::{ErrorCode, Failure};
use crate::profile_document_identity::ProfileWorkspaceIdentity;
use crate::yaml_ast_diff::YamlAstDiffSnapshot;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotRestoreTarget {
    pub profile: String,
    pub snapshot_id: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotRestoreIdentity {
    pub owner: u64,
    pub sequence: u64,
    pub snapshot_hash: String,
    pub workspace_identity: ProfileWorkspaceIdentity,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotRestoreReview {
    pub identity: SnapshotRestoreIdentity,
    pub target: SnapshotRestoreTarget,
    pub before_yaml: String,
    pub restored_yaml: String,
    pub difference: YamlAstDiffSnapshot,
}
impl SnapshotRestoreReview {
    pub fn validate(&self) -> Result<(), Failure> {
        if self.identity.owner == 0
            || self.identity.sequence == 0
            || self.target.profile.is_empty()
            || self.target.snapshot_id.is_empty()
            || self
                .identity
                .workspace_identity
                .document
                .profile_id
                .as_str()
                != self.target.profile
            || self.identity.snapshot_hash.len() != 64
            || !self
                .identity
                .snapshot_hash
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
            || self.difference.target_id != self.target.profile
            || self.difference.source_path.as_ref() != Some(&self.target.snapshot_id)
        {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "Invalid snapshot restoration review",
                false,
            ));
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotRestoreReceipt {
    pub identity: SnapshotRestoreIdentity,
    pub committed: ProfileWorkspaceIdentity,
}
impl SnapshotRestoreReceipt {
    pub fn validate(&self, identity: &SnapshotRestoreIdentity) -> Result<(), Failure> {
        if self.identity != *identity
            || self.committed.document.profile_id != identity.workspace_identity.document.profile_id
            || self.committed.document.engine_id != identity.workspace_identity.document.engine_id
            || self.committed.document.revision.as_str() != identity.snapshot_hash
            || self.committed.sidecar_revision != identity.workspace_identity.sidecar_revision
        {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "Invalid snapshot restoration receipt",
                false,
            ));
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotRestoreCancelled {
    pub identity: SnapshotRestoreIdentity,
}
