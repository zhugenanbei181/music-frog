//! Prepared exports freeze redacted shared facts until the user confirms or cancels.
use crate::log_application::LogApplication;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::log_export::{
    LogExportArtifact, LogExportIdentity, LogExportReceipt, LogExportSummary,
};
use infiltrator_domain::log_export::prepare_log_export;
use infiltrator_ports::endpoint::EndpointSource;
use infiltrator_ports::log_export::LogExportPort;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct PreparedExport {
    sequence: u64,
    artifact: Option<LogExportArtifact>,
    receipt: Option<LogExportReceipt>,
    saving: bool,
}
struct SavingGuard<'a>(&'a Mutex<PreparedExport>);
impl Drop for SavingGuard<'_> {
    fn drop(&mut self) {
        self.0.lock().expect("prepared log export").saving = false;
    }
}
#[derive(Clone)]
pub struct LogExportApplication {
    logs: LogApplication,
    port: Option<Arc<dyn LogExportPort>>,
    secrets: Arc<Vec<String>>,
    endpoint: Option<Arc<dyn EndpointSource>>,
    prepared: Arc<Mutex<PreparedExport>>,
}
impl LogExportApplication {
    pub(crate) fn belongs_to(&self, logs: &LogApplication) -> bool {
        self.logs.same_owner(logs)
    }
    pub fn new(
        logs: LogApplication,
        port: Option<Arc<dyn LogExportPort>>,
        secrets: Vec<String>,
    ) -> Self {
        Self {
            logs,
            port,
            secrets: Arc::new(secrets),
            endpoint: None,
            prepared: Arc::default(),
        }
    }
    pub fn with_endpoint_source(mut self, endpoint: Arc<dyn EndpointSource>) -> Self {
        self.endpoint = Some(endpoint);
        self
    }
    pub async fn prepare(&self) -> Result<LogExportSummary, Failure> {
        let mut secrets = self.secrets.as_ref().clone();
        if let Some(source) = &self.endpoint
            && let Some(secret) = source.resolve().await.map_err(Failure::from)?.secret
        {
            secrets.push(secret);
        }
        let (session, records) = self.logs.export_records()?;
        let mut prepared = self.prepared.lock().expect("prepared log export");
        if prepared.saving {
            return Err(busy());
        }
        prepared.sequence = prepared
            .sequence
            .checked_add(1)
            .ok_or_else(|| invalid("Log export identity exhausted"))?;
        let artifact = prepare_log_export(session, prepared.sequence, &records, &secrets)?;
        let summary = artifact.summary.clone();
        prepared.artifact = Some(artifact);
        prepared.receipt = None;
        Ok(summary)
    }
    pub fn cancel(&self, identity: &LogExportIdentity) -> Result<(), Failure> {
        let mut prepared = self.prepared.lock().expect("prepared log export");
        if prepared.saving {
            return Err(busy());
        }
        if prepared
            .artifact
            .as_ref()
            .is_some_and(|artifact| artifact.summary.identity == *identity)
        {
            prepared.artifact = None;
            prepared.receipt = None;
            Ok(())
        } else {
            Err(invalid("Log export confirmation has expired"))
        }
    }
    pub async fn save(&self, identity: &LogExportIdentity) -> Result<LogExportReceipt, Failure> {
        if !self.logs.has_export_scope(identity.session) {
            return Err(invalid(
                "Log export belongs to an earlier controller session",
            ));
        }
        let (session, _) = self.logs.export_records()?;
        if identity.session != session {
            return Err(invalid(
                "Log export belongs to an earlier controller session",
            ));
        }
        let artifact = {
            let mut prepared = self.prepared.lock().expect("prepared log export");
            if prepared.saving {
                return Err(busy());
            }
            let artifact = prepared
                .artifact
                .as_ref()
                .filter(|artifact| artifact.summary.identity == *identity)
                .ok_or_else(|| invalid("Log export confirmation has expired"))?;
            if let Some(receipt) = &prepared.receipt {
                return Ok(receipt.clone());
            }
            let artifact = artifact.clone();
            prepared.saving = true;
            artifact
        };
        let _saving = SavingGuard(&self.prepared);
        let port = self
            .port
            .as_ref()
            .ok_or_else(|| Failure::unsupported("This host has no log export destination"))?;
        let summary = artifact.summary.clone();
        let receipt = port.save(artifact).await.map_err(Failure::from)?;
        receipt.validate(&summary)?;
        let mut prepared = self.prepared.lock().expect("prepared log export");
        if prepared
            .artifact
            .as_ref()
            .is_some_and(|artifact| artifact.summary.identity == *identity)
        {
            prepared.receipt = Some(receipt.clone());
        }
        Ok(receipt)
    }
}
fn invalid(reason: &str) -> Failure {
    Failure::new(ErrorCode::InvalidState, reason, false)
}
fn busy() -> Failure {
    Failure::new(ErrorCode::NotReady, "A log export is being saved", true)
}

#[cfg(test)]
#[path = "log_export_application_tests.rs"]
mod tests;
