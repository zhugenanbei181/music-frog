//! Profile-backed controller endpoint adapter.

use crate::manager::ConfigManager;
use async_trait::async_trait;
use infiltrator_contract::profile_document_identity::ProfileId;
use infiltrator_ports::endpoint::{ControllerEndpoint, EndpointSource};
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::secure_store::SecureStore;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_ports::error::PortError;
use std::sync::Arc;
use yaml_rust2::YamlLoader;

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

    async fn profile_secret(&self, profile: &ProfileId) -> Result<Option<String>, PortError> {
        let content = self
            .config
            .load(profile.as_str())
            .await
            .map_err(|error| PortError::Io(error.to_string()))?;
        let documents = YamlLoader::load_from_str(&content)
            .map_err(|error| PortError::Io(format!("invalid profile YAML: {error}")))?;
        Ok(documents
            .first()
            .and_then(|document| document["secret"].as_str())
            .map(str::to_owned))
    }

    pub async fn resolve_profile(
        &self,
        profile: &ProfileId,
    ) -> Result<ControllerEndpoint, PortError> {
        let url = self
            .config
            .get_external_controller_for_profile(profile)
            .await
            .map_err(|error| PortError::Io(error.to_string()))?;
        let secret = self.profile_secret(profile).await?;
        Ok(ControllerEndpoint { url, secret })
    }
}

#[async_trait]
impl<S: SecureStore> EndpointSource for ProfileEndpointSource<S> {
    async fn resolve(&self) -> Result<ControllerEndpoint, PortError> {
        let profile = ProfileStore::get_current(self.config.engine_profile_store()).await?;
        let profile = ProfileId::new(profile).map_err(|error| {
            PortError::Rejected(Failure::new(
                ErrorCode::InvalidInput,
                error.to_string(),
                false,
            ))
        })?;
        self.resolve_profile(&profile).await
    }
}
