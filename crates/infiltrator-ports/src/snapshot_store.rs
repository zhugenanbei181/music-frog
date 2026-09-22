//! Runtime-neutral configuration snapshot persistence port.

use async_trait::async_trait;
use infiltrator_domain::snapshots::SnapshotMeta;
use std::path::Path;

use crate::error::PortError;

#[async_trait]
pub trait SnapshotStore: Send + Sync {
    async fn save(&self, profile: &str, content: &str) -> Result<SnapshotMeta, PortError>;
    async fn list(&self, profile: &str) -> Result<Vec<SnapshotMeta>, PortError>;
    async fn read(&self, profile: &str, path: &Path) -> Result<String, PortError>;
    /// DUAL-09-07: delete one snapshot selected by the shared prune policy.
    /// Adapters must reject identities outside the profile's history
    /// directory instead of deleting arbitrary files.
    async fn delete(&self, profile: &str, path: &Path) -> Result<(), PortError>;
}
