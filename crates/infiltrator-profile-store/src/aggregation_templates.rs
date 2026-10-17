//! Shared storage for engine-scoped aggregation-template sidecars.

use crate::store::EngineProfileStore;
use infiltrator_contract::aggregator::AggregationTemplate;
use infiltrator_ports::secure_store::SecureStore;
use lurker_core_ports::error::PortError;
use lurker_profile_store::document_store::{read_optional, remove_optional, write_document};

impl<S: SecureStore> EngineProfileStore<S> {
    /// Load the shared aggregation-template sidecar while the caller holds the
    /// profile-write boundary.
    pub async fn load_aggregation_templates_locked(
        &self,
    ) -> Result<Vec<AggregationTemplate>, PortError> {
        let Some(text) =
            read_optional(&self.layout().aggregation_templates_path(self.config_dir())).await?
        else {
            return Ok(Vec::new());
        };
        serde_yaml_ng::from_str(&text).map_err(|error| PortError::Io(error.to_string()))
    }

    /// Replace the sidecar while the caller holds profile-write and reset
    /// admission fences. An empty collection removes the sidecar.
    pub async fn save_aggregation_templates_locked(
        &self,
        templates: &[AggregationTemplate],
    ) -> Result<(), PortError> {
        let path = self.layout().aggregation_templates_path(self.config_dir());
        if templates.is_empty() {
            return remove_optional(&path).await;
        }
        let text = serde_yaml_ng::to_string(templates)
            .map_err(|error| PortError::Io(error.to_string()))?;
        write_document(&path, &text).await
    }
}
