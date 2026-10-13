//! Runtime observations that do not read or write engine configuration.

use infiltrator_contract::command::CoreLogLevel;
use infiltrator_contract::error::Failure;
use infiltrator_domain::runtime::{MemoryData, TrafficData};
use infiltrator_ports::core_log_level::CoreLogLevelPort;
use infiltrator_ports::runtime_gateway::RuntimeGateway;
use infiltrator_ports::runtime_stream::ScopedRuntimeStream;
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

    pub async fn logs(
        &self,
        level: Option<String>,
    ) -> Result<ScopedRuntimeStream<String>, Failure> {
        let scope = self.gateway.current_core_scope().map_err(Failure::from)?;
        let stream = self
            .gateway
            .runtime_stream_port()
            .logs(scope.clone(), level)
            .await
            .map_err(Failure::from)?;
        Ok(crate::runtime_stream_application::scope_checked(
            self.gateway.clone(),
            scope,
            stream,
        ))
    }

    pub async fn traffic(&self) -> Result<ScopedRuntimeStream<TrafficData>, Failure> {
        let scope = self.gateway.current_core_scope().map_err(Failure::from)?;
        let stream = self
            .gateway
            .runtime_stream_port()
            .traffic(scope.clone())
            .await
            .map_err(Failure::from)?;
        Ok(crate::runtime_stream_application::scope_checked(
            self.gateway.clone(),
            scope,
            stream,
        ))
    }
}

#[cfg(test)]
#[path = "runtime_query_application_tests.rs"]
pub(crate) mod tests;
