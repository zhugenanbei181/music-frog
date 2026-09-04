//! Filesystem adapter for the per-app routing configuration.
//!
//! The routing model, classification, process aliases and CIDR logic live in
//! `infiltrator-domain::app_routing`. This module owns only the legacy
//! `app_routing.toml` location and mutation helpers.

use infiltrator_domain::app_routing::AppRoutingConfig;
use infiltrator_ports::app_routing_store::AppRoutingStore;
use infiltrator_ports::error::PortError;
use mihomo_platform::paths::get_home_dir;
use std::path::PathBuf;

fn config_path() -> anyhow::Result<PathBuf> {
    let home = get_home_dir()?;
    Ok(home.join("app_routing.toml"))
}

pub struct FileAppRoutingStore {
    path: PathBuf,
}

impl FileAppRoutingStore {
    pub fn current() -> anyhow::Result<Self> {
        Ok(Self {
            path: config_path()?,
        })
    }
}

impl AppRoutingStore for FileAppRoutingStore {
    fn load(&self) -> Result<AppRoutingConfig, PortError> {
        if !self.path.exists() {
            return Ok(AppRoutingConfig::default());
        }
        let content = std::fs::read_to_string(&self.path)
            .map_err(|error| PortError::Io(error.to_string()))?;
        toml::from_str(&content).map_err(|error| PortError::Io(error.to_string()))
    }

    fn save(&self, config: &AppRoutingConfig) -> Result<(), PortError> {
        let content =
            toml::to_string_pretty(config).map_err(|error| PortError::Io(error.to_string()))?;
        std::fs::write(&self.path, content).map_err(|error| PortError::Io(error.to_string()))
    }
}
