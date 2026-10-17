//! Android host process evidence used by Core owner restart recovery.

use crate::error::PortError;
use async_trait::async_trait;
use infiltrator_contract::android_control::{
    AndroidCoreProcessObservation, AndroidCoreProcessRecoveryReceipt,
};
use lurker_core_contract::core_process::CoreProcessStartContext;

#[async_trait]
pub trait AndroidCoreProcessRecoveryPort: Send + Sync {
    async fn observe(&self) -> Result<AndroidCoreProcessObservation, PortError>;

    /// Clear only an exited record whose full managed context matches the
    /// proof already recovered by the application layer.
    async fn consume_exited(
        &self,
        context: &CoreProcessStartContext,
    ) -> Result<AndroidCoreProcessRecoveryReceipt, PortError>;
}
