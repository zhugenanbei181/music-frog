//! Profile-backed controller endpoint adapter.

use async_trait::async_trait;
use infiltrator_ports::endpoint::{ControllerEndpoint, EndpointSource};
use infiltrator_ports::error::PortError;
use infiltrator_ports::secure_store::SecureStore;
use std::sync::Arc;
use yaml_rust2::YamlLoader;

use crate::manager::ConfigManager;

/// Resolves the active profile's controller URL and secret on demand.
///
/// Keeping this adapter in `mihomo-config` makes profile parsing a config
/// concern; the application and lifecycle layers only see the neutral port.
pub struct ProfileEndpointSource<S: SecureStore> {
    config: Arc<ConfigManager<S>>,
}

impl<S: SecureStore> ProfileEndpointSource<S> {
    pub fn new(config: Arc<ConfigManager<S>>) -> Self {
        Self { config }
    }

    async fn current_profile_secret(&self) -> Result<Option<String>, PortError> {
        let profile = self
            .config
            .get_current()
            .await
            .map_err(|error| PortError::Io(error.to_string()))?;
        let content = self
            .config
            .load(&profile)
            .await
            .map_err(|error| PortError::Io(error.to_string()))?;
        let documents = YamlLoader::load_from_str(&content)
            .map_err(|error| PortError::Io(format!("invalid profile YAML: {error}")))?;
        Ok(documents
            .first()
            .and_then(|document| document["secret"].as_str())
            .map(str::to_owned))
    }
}

#[async_trait]
impl<S: SecureStore> EndpointSource for ProfileEndpointSource<S> {
    async fn resolve(&self) -> Result<ControllerEndpoint, PortError> {
        let url = self
            .config
            .get_external_controller()
            .await
            .map_err(|error| PortError::Io(error.to_string()))?;
        let secret = self.current_profile_secret().await?;
        Ok(ControllerEndpoint { url, secret })
    }
}
