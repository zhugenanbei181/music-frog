//! Isolated diagnostic host for real-command captures; never accesses operator files or processes.
use async_trait::async_trait;
use infiltrator_contract::doctor::{
    BootstrapReport, DoctorCheckMeta, DoctorCheckResult, DoctorFixReport, DoctorReport,
    DoctorStatus,
};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_ports::doctor::DoctorPort;
use infiltrator_ports::error::PortError;
use std::sync::atomic::{AtomicUsize, Ordering};

pub const CHECK_ID: &str = "capture.configuration";
#[derive(Default)]
pub struct DoctorCapturePort {
    runs: AtomicUsize,
}
impl DoctorCapturePort {
    pub fn run_count(&self) -> usize {
        self.runs.load(Ordering::SeqCst)
    }
}
#[async_trait]
impl DoctorPort for DoctorCapturePort {
    async fn run(&self, _: Option<String>) -> Result<DoctorReport, PortError> {
        let run = self.runs.fetch_add(1, Ordering::SeqCst);
        if run > 0 {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::Permission,
                "Allow diagnostic access, then retry this check.",
                false,
            )));
        }
        Ok(DoctorReport {
            started_at: 10,
            finished_at: 20,
            checks: vec![DoctorCheckResult {
                id: CHECK_ID.into(),
                category: "Configuration".into(),
                status: DoctorStatus::Fail,
                summary: "Configuration requires repair".into(),
                detail: Some("The observed configuration directory is missing.".into()),
                hint: Some("Repair the directory after allowing diagnostic access.".into()),
            }],
        })
    }
    async fn fix(&self, _: Option<String>) -> Result<DoctorFixReport, PortError> {
        Err(PortError::Failed(
            "capture refuses filesystem repair".into(),
        ))
    }
    fn list_checks(&self) -> Vec<DoctorCheckMeta> {
        vec![DoctorCheckMeta {
            id: CHECK_ID.into(),
            category: "Configuration".into(),
            summary: "Configuration requires repair".into(),
            why: "Inspect configuration".into(),
            fail_means: "Missing directory".into(),
            hint: "Repair after allowing access".into(),
            fixable: true,
            default_enabled: true,
        }]
    }
    fn explain(&self, id: &str) -> Result<DoctorCheckMeta, PortError> {
        self.list_checks()
            .into_iter()
            .find(|meta| meta.id == id)
            .ok_or_else(|| PortError::NotFound(id.into()))
    }
    async fn bootstrap(&self) -> Result<BootstrapReport, PortError> {
        Err(PortError::Failed(
            "capture refuses initialization effects".into(),
        ))
    }
}
