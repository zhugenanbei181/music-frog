//! Shared storage for engine-scoped named-overrides sidecar (SLINT-004).

use crate::store::EngineProfileStore;
use infiltrator_contract::named_override::NamedOverrideCatalog;
use infiltrator_ports::secure_store::SecureStore;
use lurker_core_ports::error::PortError;
use lurker_profile_store::document_store::{read_optional, remove_optional, write_document};

impl<S: SecureStore> EngineProfileStore<S> {
    pub async fn load_named_override_catalog_locked(
        &self,
    ) -> Result<NamedOverrideCatalog, PortError> {
        let Some(text) =
            read_optional(&self.layout().named_override_catalog_path(self.config_dir())).await?
        else {
            return Ok(NamedOverrideCatalog::default());
        };
        serde_yaml_ng::from_str(&text).map_err(|error| PortError::Io(error.to_string()))
    }

    pub async fn save_named_override_catalog_locked(
        &self,
        catalog: &NamedOverrideCatalog,
    ) -> Result<(), PortError> {
        let path = self.layout().named_override_catalog_path(self.config_dir());
        if catalog.overrides.is_empty() && catalog.profile_bindings.is_empty() {
            return remove_optional(&path).await;
        }
        let text =
            serde_yaml_ng::to_string(catalog).map_err(|error| PortError::Io(error.to_string()))?;
        write_document(&path, &text).await
    }
}
