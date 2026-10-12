//! Shared storage for engine-scoped aggregation-template sidecars.

use crate::document_store::{read_optional, remove_optional, write_document};
use crate::store::EngineProfileStore;
use infiltrator_contract::aggregator::AggregationTemplate;
use infiltrator_ports::error::PortError;
use infiltrator_ports::secure_store::SecureStore;
use std::path::{Path, PathBuf};

const AGGREGATION_TEMPLATES_FILE: &str = ".aggregation-templates.yaml";

impl<S: SecureStore> EngineProfileStore<S> {
    /// Load the shared aggregation-template sidecar while the caller holds the
    /// profile-write boundary.
    pub async fn load_aggregation_templates_locked(
        &self,
    ) -> Result<Vec<AggregationTemplate>, PortError> {
        let Some(text) = read_optional(&aggregation_templates_path(self.config_dir())).await?
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
        let path = aggregation_templates_path(self.config_dir());
        if templates.is_empty() {
            return remove_optional(&path).await;
        }
        let text = serde_yaml_ng::to_string(templates)
            .map_err(|error| PortError::Io(error.to_string()))?;
        write_document(&path, &text).await
    }
}

fn aggregation_templates_path(config_dir: &Path) -> PathBuf {
    config_dir.join("options").join(AGGREGATION_TEMPLATES_FILE)
}
