//! Host-owned system HTTP/SOCKS proxy capability.

use async_trait::async_trait;
use infiltrator_contract::system_proxy::{
    SystemProxyDesiredState, SystemProxyObservation, SystemProxyRecoveryReport,
    SystemProxyRecoverySnapshot,
};
use std::sync::{Arc, Mutex};

use crate::error::PortError;

#[async_trait]
pub trait SystemProxyPort: Send + Sync {
    fn shared_target(&self) -> Arc<Mutex<Option<SystemProxyDesiredState>>> {
        Arc::new(Mutex::new(None))
    }

    fn shared_recovery(&self) -> Arc<Mutex<SystemProxyRecoverySnapshot>> {
        Arc::new(Mutex::new(SystemProxyRecoverySnapshot::default()))
    }

    async fn arm_recovery(
        &self,
        _previous: SystemProxyObservation,
        _desired: SystemProxyDesiredState,
    ) -> Result<(), PortError> {
        Ok(())
    }

    async fn recover_orphaned(&self) -> Result<SystemProxyRecoveryReport, PortError> {
        Ok(SystemProxyRecoveryReport::NotNeeded)
    }

    async fn clear_recovery(&self) -> Result<(), PortError> {
        Ok(())
    }

    async fn snapshot(&self) -> Result<SystemProxyObservation, PortError>;
    async fn apply(
        &self,
        endpoint: Option<String>,
        bypass: Option<String>,
    ) -> Result<(), PortError>;
}
