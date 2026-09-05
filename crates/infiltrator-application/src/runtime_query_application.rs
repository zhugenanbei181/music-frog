//! Runtime observation use-cases over the controller gateway.

use infiltrator_contract::error::Failure;
use infiltrator_contract::command::ProxyMode;
use infiltrator_domain::runtime::{MemoryData, TrafficData};
use infiltrator_ports::runtime_gateway::{RuntimeGateway, RuntimeStream};
use std::sync::Arc;

#[derive(Clone)]
pub struct RuntimeQueryApplication {
    gateway: Arc<dyn RuntimeGateway>,
}

impl RuntimeQueryApplication {
    pub fn new(gateway: Arc<dyn RuntimeGateway>) -> Self {
        Self { gateway }
    }

    pub async fn memory(&self) -> Result<MemoryData, Failure> {
        self.gateway.get_memory().await.map_err(Failure::from)
    }

    pub async fn set_proxy_mode(&self, mode: ProxyMode) -> Result<(), Failure> {
        self.gateway
            .set_proxy_mode(mode)
            .await
            .map_err(Failure::from)
    }

    pub async fn logs(
        &self,
        level: Option<String>,
    ) -> Result<RuntimeStream<String>, Failure> {
        self.gateway
            .stream_logs(level)
            .await
            .map_err(Failure::from)
    }

    pub async fn traffic(&self) -> Result<RuntimeStream<TrafficData>, Failure> {
        self.gateway
            .stream_traffic()
            .await
            .map_err(Failure::from)
    }
}
