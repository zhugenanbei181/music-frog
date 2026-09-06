//! Host-owned system HTTP/SOCKS proxy capability.

use async_trait::async_trait;
use infiltrator_contract::system_proxy::{SystemProxyDesiredState, SystemProxyObservation};
use std::sync::{Arc, Mutex};

use crate::error::PortError;

#[async_trait]
pub trait SystemProxyPort: Send + Sync {
    fn shared_target(&self) -> Arc<Mutex<Option<SystemProxyDesiredState>>> {
        Arc::new(Mutex::new(None))
    }

    async fn snapshot(&self) -> Result<SystemProxyObservation, PortError>;
    async fn apply(
        &self,
        endpoint: Option<String>,
        bypass: Option<String>,
    ) -> Result<(), PortError>;
}
