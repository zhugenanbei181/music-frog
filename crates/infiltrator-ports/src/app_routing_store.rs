//! Persistence port for per-app routing preferences.

use infiltrator_domain::app_routing::AppRoutingConfig;

use crate::error::PortError;

pub trait AppRoutingStore: Send + Sync {
    fn load(&self) -> Result<AppRoutingConfig, PortError>;
    fn save(&self, config: &AppRoutingConfig) -> Result<(), PortError>;
}
