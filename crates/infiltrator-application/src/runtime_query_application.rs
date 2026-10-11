//! Runtime observations that do not read or write engine configuration.

use infiltrator_contract::command::CoreLogLevel;
use infiltrator_contract::error::Failure;
use infiltrator_domain::runtime::{MemoryData, TrafficData};
use infiltrator_ports::core_log_level::CoreLogLevelPort;
use infiltrator_ports::runtime_gateway::{RuntimeGateway, RuntimeStream};
use std::sync::Arc;

#[derive(Clone)]
pub struct RuntimeQueryApplication {
    gateway: Arc<dyn RuntimeGateway>,
    core_log_level_port: Option<Arc<dyn CoreLogLevelPort>>,
}

impl RuntimeQueryApplication {
    pub fn new(gateway: Arc<dyn RuntimeGateway>) -> Self {
        Self {
            gateway,
            core_log_level_port: None,
        }
    }

    pub fn with_core_log_level_port(mut self, port: Arc<dyn CoreLogLevelPort>) -> Self {
        self.core_log_level_port = Some(port);
        self
    }

    pub async fn memory(&self) -> Result<MemoryData, Failure> {
        self.gateway.get_memory().await.map_err(Failure::from)
    }

    pub async fn set_core_log_level(&self, level: CoreLogLevel) -> Result<(), Failure> {
        let Some(port) = &self.core_log_level_port else {
            return Err(Failure::unsupported(
                "core log-level control is not composed for this engine",
            ));
        };
        let observed = port
            .set_core_log_level(level)
            .await
            .map_err(Failure::from)?;
        if observed != level {
            return Err(Failure::new(
                infiltrator_contract::error::ErrorCode::InvalidState,
                format!(
                    "core log level readback mismatch: requested {}, observed {}",
                    level.as_key(),
                    observed.as_key()
                ),
                true,
            ));
        }
        Ok(())
    }

    pub async fn logs(&self, level: Option<String>) -> Result<RuntimeStream<String>, Failure> {
        self.gateway.stream_logs(level).await.map_err(Failure::from)
    }

    pub async fn traffic(&self) -> Result<RuntimeStream<TrafficData>, Failure> {
        self.gateway.stream_traffic().await.map_err(Failure::from)
    }
}

#[cfg(test)]
#[path = "runtime_query_application_tests.rs"]
pub(crate) mod tests;
