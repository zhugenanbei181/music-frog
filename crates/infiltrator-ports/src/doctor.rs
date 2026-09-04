//! Host-provided diagnostics and bootstrap port.

use async_trait::async_trait;
use infiltrator_contract::doctor::{
    BootstrapReport, DoctorCheckMeta, DoctorFixReport, DoctorReport,
};

use crate::error::PortError;

#[async_trait]
pub trait DoctorPort: Send + Sync {
    async fn run(&self, filter: Option<String>) -> Result<DoctorReport, PortError>;
    async fn fix(&self, filter: Option<String>) -> Result<DoctorFixReport, PortError>;
    fn list_checks(&self) -> Vec<DoctorCheckMeta>;
    fn explain(&self, check_id: &str) -> Result<DoctorCheckMeta, PortError>;
    async fn bootstrap(&self) -> Result<BootstrapReport, PortError>;
}
