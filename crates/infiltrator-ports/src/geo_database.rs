//! Core-scoped GeoIP/GeoSite database trigger port.

use crate::error::PortError;
use async_trait::async_trait;
use infiltrator_contract::core_identity::CoreScope;
use infiltrator_contract::error::Failure;
use infiltrator_contract::geo_database::GeoDatabaseTriggerReceipt;

#[async_trait]
pub trait GeoDatabasePort: Send + Sync {
    async fn trigger(&self, scope: CoreScope) -> Result<GeoDatabaseTriggerReceipt, PortError>;
}

pub struct UnsupportedGeoDatabasePort {
    reason: String,
}

impl UnsupportedGeoDatabasePort {
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }
}

#[async_trait]
impl GeoDatabasePort for UnsupportedGeoDatabasePort {
    async fn trigger(&self, _scope: CoreScope) -> Result<GeoDatabaseTriggerReceipt, PortError> {
        Err(PortError::Rejected(
            Failure::unsupported(self.reason.clone()),
        ))
    }
}
