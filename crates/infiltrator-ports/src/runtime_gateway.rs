//! Runtime controller operations exposed as a transport-neutral port.

use crate::connection::{ConnectionPort, UnsupportedConnectionPort};
use crate::error::PortError;
use crate::resources::{CoreResourcePort, UnsupportedCoreResourcePort};
use crate::runtime_configuration::RuntimeConfigurationPorts;
use crate::runtime_stream::{RuntimeStreamPort, UnsupportedRuntimeStreamPort};
use async_trait::async_trait;
use infiltrator_contract::capability::Capability;
use infiltrator_contract::core_identity::CoreScope;
use infiltrator_domain::runtime::{ProxyProvider, RuleProvider};

/// The controller-facing part of a running core host.
///
/// The trait is deliberately expressed only in domain values. It does not
/// expose `MihomoClient`, `reqwest`, Tokio receivers, process handles, or UI
/// task types. A desktop, Android, or iOS composition can provide its own
/// implementation without changing an inbound surface.
#[async_trait]
pub trait RuntimeGateway: Send + Sync {
    /// Resolve the active owner-authored scope for non-command observations.
    /// Core command handlers should pass their already validated request scope.
    fn current_core_scope(&self) -> Result<CoreScope, PortError> {
        Err(PortError::unsupported(
            Capability::CoreLifecycle,
            "this gateway cannot resolve the active Core scope",
        ))
    }
    /// Narrow engine configuration operations. The returned ports require a
    /// caller-supplied CoreScope and compare revision on every write.
    fn configuration_ports(&self) -> RuntimeConfigurationPorts {
        RuntimeConfigurationPorts::unsupported()
    }
    async fn switch_proxy(&self, group: &str, proxy: &str) -> Result<(), PortError>;
    async fn test_delay(&self, proxy: &str, url: &str, timeout_ms: u32) -> Result<u32, PortError>;
    async fn get_proxy_providers(&self) -> Result<Vec<ProxyProvider>, PortError>;
    async fn get_rule_providers(&self) -> Result<Vec<RuleProvider>, PortError>;
    async fn update_proxy_provider(&self, name: &str) -> Result<(), PortError>;
    async fn update_rule_provider(&self, name: &str) -> Result<(), PortError>;
    /// DUAL-11-06: the provider payload the controller publishes for `name`
    /// (`GET /providers/rules` → `payload`). mihomo serialises `payload` only
    /// for `type: inline` providers (`omitempty`), so `Ok(None)` is the honest
    /// answer for every downloaded provider and the client must not invent a
    /// rule list for it.
    async fn rule_provider_payload(&self, name: &str) -> Result<Option<Vec<String>>, PortError> {
        let _ = name;
        Err(PortError::unsupported(
            Capability::Profiles,
            "this gateway does not publish rule-provider payloads",
        ))
    }
    /// Ask the running core to refresh its GeoIP/GeoSite rule databases
    /// (`POST /upgrade/geo`). Gateways without the trigger surface a typed
    /// unsupported state instead of fabricating a success.
    async fn upgrade_geo(&self) -> Result<(), PortError> {
        Err(PortError::unsupported(
            Capability::CoreLifecycle,
            "geo database upgrade is not supported by this gateway",
        ))
    }
    /// Narrow connection-control adapter composed by the host. Hosts without
    /// a verified connection owner return an explicit unsupported port.
    fn connection_port(&self) -> std::sync::Arc<dyn ConnectionPort> {
        std::sync::Arc::new(UnsupportedConnectionPort)
    }

    fn runtime_stream_port(&self) -> std::sync::Arc<dyn RuntimeStreamPort> {
        std::sync::Arc::new(UnsupportedRuntimeStreamPort)
    }

    fn resource_port(&self) -> std::sync::Arc<dyn CoreResourcePort> {
        std::sync::Arc::new(UnsupportedCoreResourcePort)
    }
}
