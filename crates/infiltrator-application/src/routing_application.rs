//! Per-app routing use-cases over the routing persistence port.

use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_domain::app_routing::{AppRoutingConfig, AppRoutingMode, AppRoutingRule};
use infiltrator_ports::app_routing_store::AppRoutingStore;
use std::sync::Arc;

#[derive(Clone)]
pub struct RoutingApplication {
    store: Arc<dyn AppRoutingStore>,
}

impl RoutingApplication {
    pub fn new(store: Arc<dyn AppRoutingStore>) -> Self {
        Self { store }
    }

    pub fn load(&self) -> Result<AppRoutingConfig, Failure> {
        self.store.load().map_err(Failure::from)
    }

    pub fn save(&self, config: &AppRoutingConfig) -> Result<(), Failure> {
        self.store.save(config).map_err(Failure::from)
    }

    pub fn set_mode(&self, mode: AppRoutingMode) -> Result<(), Failure> {
        let mut config = self.load()?;
        config.mode = mode;
        self.save(&config)
    }

    pub fn toggle_package(&self, package: &str) -> Result<bool, Failure> {
        if package.trim().is_empty() {
            return Err(Failure::new(
                ErrorCode::InvalidInput,
                "package name is empty",
                false,
            ));
        }
        let mut config = self.load()?;
        let package = package.trim().to_string();
        let selected = if config.packages.contains(&package) {
            config.packages.remove(&package);
            false
        } else {
            config.packages.insert(package);
            true
        };
        self.save(&config)?;
        Ok(selected)
    }

    pub fn set_package_enabled(&self, package: &str, enabled: bool) -> Result<(), Failure> {
        if package.trim().is_empty() {
            return Err(Failure::new(
                ErrorCode::InvalidInput,
                "package name is empty",
                false,
            ));
        }
        let mut config = self.load()?;
        let package = package.trim().to_string();
        if enabled {
            config.packages.insert(package);
        } else {
            config.packages.remove(&package);
        }
        self.save(&config)
    }

    pub fn set_rule(&self, package: &str, rule: AppRoutingRule) -> Result<(), Failure> {
        let package = package.trim();
        if package.is_empty() {
            return Err(Failure::new(
                ErrorCode::InvalidInput,
                "package name is empty",
                false,
            ));
        }
        let mut config = self.load()?;
        config.rules.insert(package.to_string(), rule);
        self.save(&config)
    }

    pub fn allowed_packages(&self) -> Result<Option<Vec<String>>, Failure> {
        Ok(self.load()?.get_allowed_packages())
    }
}
