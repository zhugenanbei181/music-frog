//! Mihomo controller readiness adapter.
//!
//! The HTTP/WebSocket implementation remains Tokio-backed here, but the
//! application layer sees only the runtime-neutral `CoreReadiness` port.

use crate::client::MihomoClient;
use infiltrator_ports::core_process::CoreReadiness;
use infiltrator_ports::error::PortError;

/// Probes a controller endpoint by asking the live core for its version.
#[derive(Clone)]
pub struct ControllerReadiness {
    endpoint: String,
    secret: Option<String>,
}

impl ControllerReadiness {
    pub fn new(endpoint: impl Into<String>, secret: Option<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            secret,
        }
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }
}

#[async_trait::async_trait]
impl CoreReadiness for ControllerReadiness {
    async fn probe(&self) -> Result<String, PortError> {
        let client = MihomoClient::new(&self.endpoint, self.secret.clone())
            .map_err(|error| PortError::Network(error.to_string()))?;
        client
            .get_version()
            .await
            .map_err(|error| PortError::Network(error.to_string()))?;
        Ok(self.endpoint.clone())
    }
}
