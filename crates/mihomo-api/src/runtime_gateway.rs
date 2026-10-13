//! RuntimeGateway implementation for the HTTP controller client.

use crate::client::MihomoClient;
use crate::connection_port::MihomoConnectionPort;
use crate::error::MihomoError;
use crate::resource_port::MihomoRuntimeResourcePort;
use crate::runtime_stream::MihomoRuntimeStreamPort;
use async_trait::async_trait;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_ports::connection::ConnectionPort;
use infiltrator_ports::error::PortError;
use infiltrator_ports::resources::CoreResourcePort;
use infiltrator_ports::runtime_gateway::RuntimeGateway;
use infiltrator_ports::runtime_stream::RuntimeStreamPort;

#[async_trait]
impl RuntimeGateway for MihomoClient {
    fn current_core_scope(
        &self,
    ) -> Result<infiltrator_contract::core_identity::CoreScope, PortError> {
        self.configuration_scope_resolver().ok_or_else(|| {
            PortError::unsupported(
                infiltrator_contract::capability::Capability::CoreLifecycle,
                "Mihomo client has no Core scope resolver",
            )
        })?()
    }

    fn configuration_ports(
        &self,
    ) -> infiltrator_ports::runtime_configuration::RuntimeConfigurationPorts {
        crate::runtime_configuration::MihomoRuntimeConfigurationAdapter::ports(
            self.clone(),
            self.configuration_scope_resolver(),
        )
    }

    fn connection_port(&self) -> std::sync::Arc<dyn ConnectionPort> {
        std::sync::Arc::new(MihomoConnectionPort::new(
            self.clone(),
            self.configuration_scope_resolver(),
        ))
    }

    fn runtime_stream_port(&self) -> std::sync::Arc<dyn RuntimeStreamPort> {
        std::sync::Arc::new(MihomoRuntimeStreamPort::new(
            self.clone(),
            self.configuration_scope_resolver(),
        ))
    }

    fn resource_port(&self) -> std::sync::Arc<dyn CoreResourcePort> {
        std::sync::Arc::new(MihomoRuntimeResourcePort::new(
            self.clone(),
            self.configuration_scope_resolver(),
        ))
    }
}

pub(crate) fn network_error(error: MihomoError) -> PortError {
    if let MihomoError::Http(http) = &error {
        match http.status().map(|status| status.as_u16()) {
            Some(400 | 422) => {
                return PortError::Rejected(Failure::new(
                    ErrorCode::InvalidInput,
                    error.to_string(),
                    false,
                ));
            }
            Some(401) => {
                return PortError::Rejected(Failure::new(
                    ErrorCode::Authentication,
                    error.to_string(),
                    false,
                ));
            }
            Some(403) => return PortError::PermissionDenied(error.to_string()),
            Some(404) => return PortError::NotFound(error.to_string()),
            _ => {}
        }
    }
    PortError::Network(error.to_string())
}
