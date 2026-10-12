//! Composition adapter exposing the shared engine-scoped ProfileStore.

use crate::settings_io::app_config_manager;
use infiltrator_ports::profile_store::ProfileStore;
use std::path::PathBuf;
use std::sync::Arc;

/// Construct the default profile store for the current host.
pub async fn open() -> anyhow::Result<Arc<dyn ProfileStore>> {
    Ok(app_config_manager().await?.shared_engine_profile_store())
}

/// Resolve the same configs directory used by the profile store.
pub async fn config_dir() -> anyhow::Result<PathBuf> {
    let store = open().await?;
    Ok(store.config_dir())
}
