//! Shared storage for engine-scoped named-overrides sidecar (SLINT-004).

use crate::document_store::{read_optional, remove_optional, write_document};
use crate::store::EngineProfileStore;
use infiltrator_contract::named_override::NamedOverrideCatalog;
use infiltrator_ports::error::PortError;
use infiltrator_ports::secure_store::SecureStore;
use std::path::{Path, PathBuf};

const NAMED_OVERRIDES_FILE: &str = ".named-overrides.yaml";

impl<S: SecureStore> EngineProfileStore<S> {
    pub async fn load_named_override_catalog_locked(
        &self,
    ) -> Result<NamedOverrideCatalog, PortError> {
        let Some(text) = read_optional(&named_overrides_path(self.config_dir())).await? else {
            return Ok(NamedOverrideCatalog::default());
        };
        serde_yaml_ng::from_str(&text).map_err(|error| PortError::Io(error.to_string()))
    }

    pub async fn save_named_override_catalog_locked(
        &self,
        catalog: &NamedOverrideCatalog,
    ) -> Result<(), PortError> {
        let path = named_overrides_path(self.config_dir());
        if catalog.overrides.is_empty() && catalog.profile_bindings.is_empty() {
            return remove_optional(&path).await;
        }
        let text =
            serde_yaml_ng::to_string(catalog).map_err(|error| PortError::Io(error.to_string()))?;
        write_document(&path, &text).await
    }
}

fn named_overrides_path(config_dir: &Path) -> PathBuf {
    config_dir.join("options").join(NAMED_OVERRIDES_FILE)
}
