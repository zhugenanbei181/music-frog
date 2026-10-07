//! Host fixture performs diagnostic work and returns genuine port outcomes.
use async_trait::async_trait;
use infiltrator_contract::doctor::{
    BootstrapReport, DoctorCheckMeta, DoctorCheckResult, DoctorFixReport, DoctorReport,
    DoctorStatus,
};
use infiltrator_ports::doctor::DoctorPort;
use infiltrator_ports::error::PortError;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
#[derive(Default)]
pub struct DiagnosticPort {
    pub deny: AtomicBool,
    pub fixed: AtomicBool,
    pub runs: AtomicUsize,
    pub fixes: AtomicUsize,
    pub bootstraps: AtomicUsize,
}
#[async_trait]
impl DoctorPort for DiagnosticPort {
    async fn run(&self, _: Option<String>) -> Result<DoctorReport, PortError> {
        self.runs.fetch_add(1, Ordering::SeqCst);
        if self.deny.load(Ordering::SeqCst) {
            return Err(PortError::PermissionDenied(
                "allow diagnostic access".into(),
            ));
        }
        Ok(DoctorReport {
            started_at: 10,
            finished_at: 20,
            checks: vec![DoctorCheckResult {
                id: "config.integrity".into(),
                category: "configuration".into(),
                status: if self.fixed.load(Ordering::SeqCst) {
                    DoctorStatus::Pass
                } else {
                    DoctorStatus::Fail
                },
                summary: "observed configuration integrity".into(),
                detail: Some("finding {count}".into()),
                hint: Some("repair configuration".into()),
            }],
        })
    }
    async fn fix(&self, filter: Option<String>) -> Result<DoctorFixReport, PortError> {
        assert!(filter.is_none() || filter.as_deref() == Some("config.integrity"));
        self.fixes.fetch_add(1, Ordering::SeqCst);
        self.fixed.store(true, Ordering::SeqCst);
        Ok(DoctorFixReport::default())
    }
    fn list_checks(&self) -> Vec<DoctorCheckMeta> {
        vec![DoctorCheckMeta {
            id: "config.integrity".into(),
            category: "configuration".into(),
            summary: "integrity".into(),
            why: "inspect configuration".into(),
            fail_means: "damaged".into(),
            hint: "repair".into(),
            fixable: true,
            default_enabled: true,
        }]
    }
    fn explain(&self, id: &str) -> Result<DoctorCheckMeta, PortError> {
        self.list_checks()
            .into_iter()
            .find(|meta| meta.id == id)
            .ok_or_else(|| PortError::Failed("unknown check".into()))
    }
    async fn bootstrap(&self) -> Result<BootstrapReport, PortError> {
        self.bootstraps.fetch_add(1, Ordering::SeqCst);
        Ok(BootstrapReport::default())
    }
}
