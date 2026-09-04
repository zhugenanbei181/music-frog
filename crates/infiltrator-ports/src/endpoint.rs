use crate::error::PortError;
use async_trait::async_trait;

/// Controller URL plus its authentication material. Secrets are carried only
/// between an outbound adapter and its private client; they never enter a UI
/// projection or process handle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControllerEndpoint {
    pub url: String,
    pub secret: Option<String>,
}

/// Resolves the active controller without coupling callers to profile files or
/// a concrete HTTP client.
#[async_trait]
pub trait EndpointSource: Send + Sync {
    async fn resolve(&self) -> Result<ControllerEndpoint, PortError>;
}
