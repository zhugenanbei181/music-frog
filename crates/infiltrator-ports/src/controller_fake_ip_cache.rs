//! Scoped controller operation for clearing a running core's Fake-IP table.

use async_trait::async_trait;
use infiltrator_contract::dns_cache::DnsCacheOperationId;
use lurker_core_contract::identity::CoreScope;
use lurker_core_ports::error::PortError;

/// One controller Fake-IP flush tied to the Core owner activation and command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControllerFakeIpCacheFlushRequest {
    pub scope: CoreScope,
    pub operation: DnsCacheOperationId,
}

/// The controller's terminal response to a scoped Fake-IP flush.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControllerFakeIpCacheFlushState {
    Flushed,
}

/// Adapter-authored receipt correlated with the request that caused it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControllerFakeIpCacheFlushReceipt {
    pub scope: CoreScope,
    pub operation: DnsCacheOperationId,
    pub state: ControllerFakeIpCacheFlushState,
}

/// Clears the Fake-IP table inside the active controller-owned core.
///
/// This is separate from host cache ports: implementations must only report
/// `Flushed` after the controller confirms the operation for `request.scope`.
#[async_trait]
pub trait ControllerFakeIpCachePort: Send + Sync {
    async fn flush(
        &self,
        request: ControllerFakeIpCacheFlushRequest,
    ) -> Result<ControllerFakeIpCacheFlushReceipt, PortError>;
}
