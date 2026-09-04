//! Doctor and bootstrap use-cases over a host-provided port.

use infiltrator_contract::doctor::{
    BootstrapReport, DoctorCheckMeta, DoctorFixReport, DoctorReport,
};
use infiltrator_contract::error::Failure;
use infiltrator_ports::doctor::DoctorPort;
use std::sync::Arc;

#[derive(Clone)]
pub struct DoctorApplication {
    port: Arc<dyn DoctorPort>,
}

impl DoctorApplication {
    pub fn new(port: Arc<dyn DoctorPort>) -> Self {
        Self { port }
    }

    pub async fn run(&self, filter: Option<String>) -> Result<DoctorReport, Failure> {
        self.port.run(filter).await.map_err(Failure::from)
    }

    pub async fn fix(&self, filter: Option<String>) -> Result<DoctorFixReport, Failure> {
        self.port.fix(filter).await.map_err(Failure::from)
    }

    pub fn list_checks(&self) -> Vec<DoctorCheckMeta> {
        self.port.list_checks()
    }

    pub fn explain(&self, check_id: &str) -> Result<DoctorCheckMeta, Failure> {
        self.port.explain(check_id).map_err(Failure::from)
    }

    pub async fn bootstrap(&self) -> Result<BootstrapReport, Failure> {
        self.port.bootstrap().await.map_err(Failure::from)
    }
}
