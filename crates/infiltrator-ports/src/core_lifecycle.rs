use crate::error::PortError;
use async_trait::async_trait;
use infiltrator_contract::session::SessionToken;
use infiltrator_contract::snapshot::CoreLifecycle;
use std::time::Duration;

/// Application-facing lifecycle capability used by config transactions and
/// inbound adapters. It contains no session implementation or executor type.
#[async_trait]
pub trait CoreLifecyclePort: Send + Sync {
    fn lifecycle(&self) -> CoreLifecycle;
    fn generation(&self) -> u64;
    fn session_token(&self) -> Option<SessionToken>;

    async fn start(&self) -> Result<u64, PortError>;
    async fn stop(&self) -> Result<(), PortError>;
    async fn restart(&self) -> Result<u64, PortError>;
    async fn wait_for_ready(&self, generation: u64, timeout: Duration) -> Result<(), PortError>;

    /// Fence a readiness result against both ordering and session identity.
    async fn wait_for_ready_session(
        &self,
        generation: u64,
        session_token: SessionToken,
        timeout: Duration,
    ) -> Result<(), PortError> {
        if self.session_token() != Some(session_token) {
            return Err(PortError::Failed("stale core session token".to_string()));
        }
        self.wait_for_ready(generation, timeout).await
    }
}
