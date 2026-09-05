use crate::error::PortError;
use async_trait::async_trait;
use infiltrator_contract::session::SessionToken;
use infiltrator_contract::snapshot::{CoreLifecycle, CoreLifecycleSnapshot};
use std::time::Duration;

/// Application-facing lifecycle capability used by config transactions and
/// inbound adapters. It contains no session implementation or executor type.
#[async_trait]
pub trait CoreLifecyclePort: Send + Sync {
    fn lifecycle(&self) -> CoreLifecycle;
    fn generation(&self) -> u64;
    fn session_token(&self) -> Option<SessionToken>;

    /// Canonical lifecycle read model consumed by both UI adapters. Legacy
    /// hosts get a safe projection from the primitive methods; the shared
    /// CoreApplication supplies its real revision/failure fields.
    fn lifecycle_snapshot(&self) -> CoreLifecycleSnapshot {
        CoreLifecycleSnapshot {
            lifecycle: self.lifecycle(),
            generation: self.generation(),
            session_token: self.session_token(),
            revision: 0,
            failure: None,
        }
    }

    async fn start(&self) -> Result<u64, PortError>;
    async fn stop(&self) -> Result<(), PortError>;
    async fn restart(&self) -> Result<u64, PortError>;

    /// Mark a hot-reload transaction as in flight and return the session it
    /// is allowed to update. The default keeps lightweight legacy adapters
    /// source-compatible while still fencing the token.
    fn begin_reload(&self) -> Result<SessionToken, PortError> {
        if !matches!(self.lifecycle(), CoreLifecycle::Ready | CoreLifecycle::Running) {
            return Err(PortError::Failed(
                "core is not ready for hot reload".to_string(),
            ));
        }
        self.session_token()
            .ok_or_else(|| PortError::Failed("core session is not active".to_string()))
    }

    /// Complete a hot reload only for the session that started it.
    fn complete_reload(&self, session_token: SessionToken) -> Result<(), PortError> {
        if self.session_token() == Some(session_token) {
            Ok(())
        } else {
            Err(PortError::Failed("stale core session token".to_string()))
        }
    }

    /// Record a reload failure only for the session that started it.
    fn fail_reload(
        &self,
        session_token: SessionToken,
        _error: String,
    ) -> Result<(), PortError> {
        if self.session_token() == Some(session_token) {
            Ok(())
        } else {
            Err(PortError::Failed("stale core session token".to_string()))
        }
    }

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
