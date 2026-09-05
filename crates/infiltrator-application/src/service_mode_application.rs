//! Service-mode privilege use-cases over a host-provided port.

use infiltrator_contract::error::Failure;
use infiltrator_contract::service_mode::ServiceModeSnapshot;
use infiltrator_ports::service_mode::ServiceModePort;
use std::sync::Arc;

#[derive(Clone)]
pub struct ServiceModeApplication {
    port: Arc<dyn ServiceModePort>,
}

impl ServiceModeApplication {
    pub fn new(port: Arc<dyn ServiceModePort>) -> Self {
        Self { port }
    }

    pub async fn snapshot(&self) -> Result<ServiceModeSnapshot, Failure> {
        self.port.snapshot().await.map_err(Failure::from)
    }

    pub async fn prepare(&self) -> Result<ServiceModeSnapshot, Failure> {
        self.port.prepare().await.map_err(Failure::from)
    }
}
