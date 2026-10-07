//! Persistence port for per-app routing preferences.

use crate::error::PortError;
use infiltrator_domain::app_routing::AppRoutingConfig;

pub trait AppRoutingStore: Send + Sync {
    fn load(&self) -> Result<AppRoutingConfig, PortError>;
    fn save(&self, config: &AppRoutingConfig) -> Result<(), PortError>;
}
