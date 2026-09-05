//! Port-conflict use-cases over a host-provided adapter.

use infiltrator_contract::error::Failure;
use infiltrator_contract::port_conflict::PortConflictSnapshot;
use infiltrator_ports::port_conflict::PortConflictPort;
use std::sync::Arc;

#[derive(Clone)]
pub struct PortConflictApplication {
    port: Arc<dyn PortConflictPort>,
}

impl PortConflictApplication {
    pub fn new(port: Arc<dyn PortConflictPort>) -> Self {
        Self { port }
    }

    pub async fn snapshot(&self) -> Result<PortConflictSnapshot, Failure> {
        self.port.snapshot().await.map_err(Failure::from)
    }

    pub async fn repair(&self) -> Result<PortConflictSnapshot, Failure> {
        self.port.repair().await.map_err(Failure::from)
    }
}
