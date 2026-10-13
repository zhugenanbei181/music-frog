//! Runtime controller operations exposed as a transport-neutral port.

use crate::connection::{ConnectionPort, UnsupportedConnectionPort};
use crate::error::PortError;
use crate::resources::{CoreResourcePort, UnsupportedCoreResourcePort};
use crate::runtime_configuration::RuntimeConfigurationPorts;
use crate::runtime_stream::{RuntimeStreamPort, UnsupportedRuntimeStreamPort};
use async_trait::async_trait;
use infiltrator_contract::capability::Capability;
use infiltrator_contract::core_identity::CoreScope;

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
