//! Runtime controller operations exposed as a transport-neutral port.

use crate::error::PortError;
use crate::profile_workspace::{ProfileWorkspace, ProfileWorkspaceUpdate};
use crate::runtime_configuration::RuntimeConfigurationPorts;
use async_trait::async_trait;
use futures_util::stream::BoxStream;
use infiltrator_contract::capability::Capability;
use infiltrator_contract::core_identity::CoreScope;
use infiltrator_contract::core_identity::EngineId;
use infiltrator_contract::error::Failure;
use infiltrator_contract::profile_document_identity::ProfileWorkspaceIdentity;
use infiltrator_domain::apply::ApplyStrategy;
use infiltrator_domain::runtime::{
    ConnectionSnapshot, MemoryData, ProxyProvider, RuleProvider, TrafficData,
};

/// Lifecycle-independent events from a controller stream.
#[derive(Debug)]
pub enum RuntimeStreamEvent<T> {
    Connecting,
    Connected,
    Item(T),
    Reconnecting(Failure),
    Failed(Failure),
}

pub type RuntimeStream<T> = BoxStream<'static, RuntimeStreamEvent<T>>;

/// A controller event stamped by a gateway that verified its active core scope.
#[derive(Debug)]
pub struct ScopedRuntimeStreamEvent<T> {
    pub scope: CoreScope,
    pub event: RuntimeStreamEvent<T>,
}

pub type ScopedRuntimeStream<T> = BoxStream<'static, ScopedRuntimeStreamEvent<T>>;

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
    async fn get_connections(&self) -> Result<ConnectionSnapshot, PortError>;
    async fn get_memory(&self) -> Result<MemoryData, PortError>;
    /// Host-side CPU usage for the running core, when the platform exposes it.
    async fn get_cpu_percent(&self) -> Result<Option<f32>, PortError> {
        Ok(None)
    }
    /// Ask the running core to perform its supported active garbage collection.
    async fn trigger_gc(&self) -> Result<(), PortError> {
        Err(PortError::unsupported(
            Capability::CoreLifecycle,
            "core garbage collection is not supported by this gateway",
        ))
    }
    async fn close_connection(&self, id: &str) -> Result<(), PortError>;
    async fn close_all_connections(&self) -> Result<(), PortError>;

    async fn stream_logs(&self, level: Option<String>) -> Result<RuntimeStream<String>, PortError>;
    /// Opens logs only after the gateway proves it currently addresses `scope`.
    /// The default deliberately refuses to wrap the legacy unscoped stream:
    /// caller-side tagging would make stale events appear current.
    async fn stream_logs_scoped(
        &self,
        _scope: CoreScope,
        _level: Option<String>,
    ) -> Result<ScopedRuntimeStream<String>, PortError> {
        Err(PortError::unsupported(
            Capability::Logs,
            "this gateway cannot prove the requested core scope",
        ))
    }
    async fn stream_traffic(&self) -> Result<RuntimeStream<TrafficData>, PortError>;
    async fn stream_connections(&self) -> Result<RuntimeStream<ConnectionSnapshot>, PortError>;
}

/// Additional host-owned operations available when a gateway also owns the
/// running core process and its configuration transaction.
#[async_trait]
pub trait ManagedRuntime: RuntimeGateway {
    /// Engine currently owned by this runtime; absence denies a profile apply.
    fn engine_id(&self) -> Option<EngineId>;
    fn generation(&self) -> u64;
    async fn is_running(&self) -> bool;
    async fn restart(&self) -> Result<u64, PortError>;
    async fn http_proxy_endpoint(&self) -> Result<Option<String>, PortError>;
    async fn shutdown(&self) -> Result<(), PortError>;
    async fn apply_current_config(&self, strategy: ApplyStrategy) -> Result<u64, PortError>;
    async fn apply_profile_content(
        &self,
        content: &str,
        strategy: ApplyStrategy,
    ) -> Result<u64, PortError>;
    async fn apply_profile_workspace(
        &self,
        _expected: &ProfileWorkspaceIdentity,
        _update: &ProfileWorkspaceUpdate,
        _strategy: ApplyStrategy,
    ) -> Result<ProfileWorkspace, PortError> {
        Err(PortError::unsupported(
            Capability::Profiles,
            "source-bound profile/options apply is unavailable",
        ))
    }
}
