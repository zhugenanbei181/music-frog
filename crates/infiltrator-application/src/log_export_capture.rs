//! A deliberately read-only export destination exercises the actual command failure path.
use async_trait::async_trait;
use infiltrator_contract::log_export::{LogExportArtifact, LogExportReceipt};
use infiltrator_ports::error::PortError;
use infiltrator_ports::log_export::LogExportPort;
pub struct ReadOnlyLogExportCapture;
#[async_trait]
impl LogExportPort for ReadOnlyLogExportCapture {
    async fn save(&self, _: LogExportArtifact) -> Result<LogExportReceipt, PortError> {
        Err(PortError::PermissionDenied(
            "The isolated log export destination is read-only".into(),
        ))
    }
}
