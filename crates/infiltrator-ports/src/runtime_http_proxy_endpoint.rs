//! Typed observation of the host's local HTTP proxy endpoint.

use async_trait::async_trait;
use lurker_core_ports::error::PortError;

/// Whether a usable local HTTP proxy endpoint is configured for the active
/// runtime. Absence is a domain state, not `None` or a synthetic endpoint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuntimeHttpProxyEndpoint {
    Configured { endpoint: String },
    NotConfigured,
}

/// Narrow host port for consumers that need a local HTTP proxy endpoint.
#[async_trait]
pub trait RuntimeHttpProxyEndpointPort: Send + Sync {
    async fn observe_http_proxy_endpoint(&self) -> Result<RuntimeHttpProxyEndpoint, PortError>;
}
