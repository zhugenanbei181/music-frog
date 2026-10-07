//! The host owns the export destination and durable file publication.
use crate::error::PortError;
use async_trait::async_trait;
use infiltrator_contract::log_export::{LogExportArtifact, LogExportReceipt};

#[async_trait]
pub trait LogExportPort: Send + Sync {
    /// Publish complete bytes without replacing an existing file. Errors must
    /// leave no partial artifact; success includes the actual path and byte count.
    async fn save(&self, artifact: LogExportArtifact) -> Result<LogExportReceipt, PortError>;
}
