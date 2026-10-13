//! RuntimeGateway implementation for the HTTP controller client.

use crate::client::MihomoClient;
use crate::connection_port::MihomoConnectionPort;
use crate::error::MihomoError;
use crate::resource_port::MihomoRuntimeResourcePort;
use crate::runtime_stream::MihomoRuntimeStreamPort;
use async_trait::async_trait;
use infiltrator_contract::capability::Capability;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_domain::runtime::{MemoryData, ProxyProvider, RuleProvider};
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

    async fn switch_proxy(&self, group: &str, proxy: &str) -> Result<(), PortError> {
        MihomoClient::switch_proxy(self, group, proxy)
            .await
            .map_err(network_error)
    }

    async fn test_delay(&self, proxy: &str, url: &str, timeout_ms: u32) -> Result<u32, PortError> {
        MihomoClient::test_delay(self, proxy, url, timeout_ms)
            .await
            .map_err(network_error)
    }

    async fn get_proxy_providers(&self) -> Result<Vec<ProxyProvider>, PortError> {
        MihomoClient::get_proxy_providers(self)
            .await
            .map(|providers| providers.into_values().map(Into::into).collect())
            .map_err(network_error)
    }

    async fn get_rule_providers(&self) -> Result<Vec<RuleProvider>, PortError> {
        MihomoClient::get_rule_providers(self)
            .await
            .map(|providers| providers.into_values().map(Into::into).collect())
            .map_err(network_error)
    }

    async fn update_proxy_provider(&self, name: &str) -> Result<(), PortError> {
        MihomoClient::update_proxy_provider(self, name)
            .await
            .map_err(network_error)
    }

    async fn update_rule_provider(&self, name: &str) -> Result<(), PortError> {
        MihomoClient::update_rule_provider(self, name)
            .await
            .map_err(network_error)
    }

    async fn rule_provider_payload(&self, name: &str) -> Result<Option<Vec<String>>, PortError> {
        MihomoClient::get_rule_provider_payload(self, name)
            .await
            .map_err(network_error)
    }

    async fn get_memory(&self) -> Result<MemoryData, PortError> {
        MihomoClient::get_memory(self)
            .await
            .map(Into::into)
            .map_err(network_error)
    }

    async fn trigger_gc(&self) -> Result<(), PortError> {
        match MihomoClient::trigger_gc(self).await {
            Ok(()) => Ok(()),
            Err(MihomoError::Http(error))
                if error.status().is_some_and(|status| status.as_u16() == 404) =>
            {
                Err(PortError::unsupported(
                    Capability::CoreLifecycle,
                    "Mihomo exposes /debug/gc only when log-level is debug at process startup",
                ))
            }
            Err(error) => Err(network_error(error)),
        }
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
