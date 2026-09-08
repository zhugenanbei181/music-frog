//! Runtime-neutral privileged network transaction and rollback use-case.

use futures_util::lock::Mutex;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::privileged_network::{
    PrivilegedNetworkRequest, PrivilegedNetworkSnapshot, PrivilegedNetworkState,
};
use infiltrator_ports::error::PortError;
use infiltrator_ports::privileged_network::PrivilegedNetworkPort;
use std::sync::Arc;

struct State {
    snapshot: PrivilegedNetworkSnapshot,
    next_revision: u64,
}

/// Runs an explicit host-injection transaction and guarantees a cleanup
/// readback before reporting success. It is useful for headless host tests and
/// for a future privileged preflight UI; it never manufactures host support.
#[derive(Clone)]
pub struct PrivilegedNetworkApplication {
    port: Arc<dyn PrivilegedNetworkPort>,
    state: Arc<Mutex<State>>,
}

impl PrivilegedNetworkApplication {
    pub fn new(port: Arc<dyn PrivilegedNetworkPort>) -> Self {
        Self {
            port,
            state: Arc::new(Mutex::new(State {
                snapshot: PrivilegedNetworkSnapshot::default(),
                next_revision: 1,
            })),
        }
    }

    pub async fn snapshot(&self) -> PrivilegedNetworkSnapshot {
        match self.port.snapshot().await {
            Ok(snapshot) => {
                let local = self.state.lock().await.snapshot.clone();
                if matches!(local.state, PrivilegedNetworkState::Failed { .. }) {
                    local
                } else {
                    self.store(snapshot).await
                }
            }
            Err(PortError::Unsupported { reason, .. }) => {
                self.store(PrivilegedNetworkSnapshot::unsupported(
                    self.next_revision().await,
                    reason.clone(),
                ))
                .await
            }
            Err(error) => {
                let failure = Failure::from(error);
                self.store(PrivilegedNetworkSnapshot::failed(
                    self.next_revision().await,
                    failure,
                ))
                .await
            }
        }
    }

    /// Inject, read back, clean up, and read back again. Any failed injection
    /// or mismatched readback attempts cleanup and preserves a typed failure.
    pub async fn run(
        &self,
        request: PrivilegedNetworkRequest,
    ) -> Result<PrivilegedNetworkSnapshot, Failure> {
        infiltrator_domain::privileged_network_policy::validate_request(&request)
            .map_err(|message| Failure::new(ErrorCode::InvalidInput, message, false))?;
        let operation_count = request.operations.len();
        let injected = match self.port.inject(request).await {
            Ok(snapshot) => snapshot,
            Err(PortError::Unsupported { reason, .. }) => {
                let failure = Failure::unsupported(reason.clone());
                self.store(PrivilegedNetworkSnapshot::unsupported(
                    self.next_revision().await,
                    reason,
                ))
                .await;
                return Err(failure);
            }
            Err(error) => {
                let failure = Failure::from(error);
                return Err(self.rollback(failure, operation_count).await);
            }
        };
        let observed = match self.port.snapshot().await {
            Ok(snapshot) => snapshot,
            Err(error) => {
                let failure = Failure::from(error);
                return Err(self.rollback(failure, operation_count).await);
            }
        };
        if !is_active(&injected) || !is_active(&observed) {
            let failure = Failure::new(
                ErrorCode::InvalidState,
                format!(
                    "privileged network injection readback mismatch: injected={:?}, observed={:?}",
                    injected.state, observed.state
                ),
                true,
            );
            return Err(self.rollback(failure, operation_count).await);
        }

        let cleaned = match self.port.cleanup().await {
            Ok(snapshot) => snapshot,
            Err(error) => {
                let failure = Failure::from(error);
                self.store(PrivilegedNetworkSnapshot::failed_after_injection(
                    self.next_revision().await,
                    operation_count,
                    true,
                    false,
                    failure.clone(),
                ))
                .await;
                return Err(failure);
            }
        };
        let cleanup_observed = match self.port.snapshot().await {
            Ok(snapshot) => snapshot,
            Err(error) => {
                let failure = Failure::from(error);
                self.store(PrivilegedNetworkSnapshot::failed_after_injection(
                    self.next_revision().await,
                    operation_count,
                    true,
                    false,
                    failure.clone(),
                ))
                .await;
                return Err(failure);
            }
        };
        if !is_clean(&cleaned) || !is_clean(&cleanup_observed) {
            let failure = Failure::new(
                ErrorCode::InvalidState,
                format!(
                    "privileged network cleanup readback mismatch: reported={:?}, observed={:?}",
                    cleaned.state, cleanup_observed.state
                ),
                true,
            );
            self.store(PrivilegedNetworkSnapshot::failed_after_injection(
                self.next_revision().await,
                operation_count,
                true,
                false,
                failure.clone(),
            ))
            .await;
            return Err(failure);
        }
        Ok(self.store(cleanup_observed).await)
    }

    pub async fn cleanup(&self) -> Result<PrivilegedNetworkSnapshot, Failure> {
        let reported = self.port.cleanup().await.map_err(Failure::from)?;
        let observed = self.port.snapshot().await.map_err(Failure::from)?;
        if !is_clean(&reported) || !is_clean(&observed) {
            let failure = Failure::new(
                ErrorCode::InvalidState,
                format!(
                    "privileged network cleanup did not settle: reported={:?}, observed={:?}",
                    reported.state, observed.state
                ),
                true,
            );
            self.store(PrivilegedNetworkSnapshot::failed(
                self.next_revision().await,
                failure.clone(),
            ))
            .await;
            return Err(failure);
        }
        Ok(self.store(observed).await)
    }

    async fn rollback(&self, primary: Failure, operation_count: usize) -> Failure {
        let cleanup = self.port.cleanup().await;
        let observed = self.port.snapshot().await;
        match (cleanup, observed) {
            (Ok(cleanup), Ok(observed)) if is_clean(&cleanup) && is_clean(&observed) => {
                self.store(PrivilegedNetworkSnapshot::cleaned(
                    self.next_revision().await,
                    observed.operation_count.max(operation_count),
                    true,
                ))
                .await;
                primary
            }
            (cleanup, observed) => {
                let detail = format!(
                    "primary failure: {}; rollback cleanup={:?}, readback={:?}",
                    primary.message, cleanup, observed
                );
                let failure = Failure::new(ErrorCode::InvalidState, detail, true);
                self.store(PrivilegedNetworkSnapshot::failed_after_injection(
                    self.next_revision().await,
                    operation_count,
                    true,
                    true,
                    failure.clone(),
                ))
                .await;
                failure
            }
        }
    }

    async fn next_revision(&self) -> u64 {
        self.state.lock().await.next_revision
    }

    async fn store(&self, mut snapshot: PrivilegedNetworkSnapshot) -> PrivilegedNetworkSnapshot {
        let mut state = self.state.lock().await;
        snapshot.revision = state.next_revision;
        state.next_revision = state.next_revision.saturating_add(1);
        state.snapshot = snapshot.clone();
        snapshot
    }
}

fn is_active(snapshot: &PrivilegedNetworkSnapshot) -> bool {
    snapshot.state == PrivilegedNetworkState::Active && snapshot.injected
}

fn is_clean(snapshot: &PrivilegedNetworkSnapshot) -> bool {
    snapshot.state == PrivilegedNetworkState::Cleaned && !snapshot.injected
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use infiltrator_contract::privileged_network::{
        PrivilegedNetworkOperation, PrivilegedNetworkRequest,
    };
    use infiltrator_ports::error::PortError;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    struct MockPort {
        snapshot: std::sync::Mutex<PrivilegedNetworkSnapshot>,
        inject_calls: AtomicUsize,
        cleanup_calls: AtomicUsize,
        fail_inject: AtomicBool,
        fail_cleanup: AtomicBool,
    }

    #[async_trait]
    impl PrivilegedNetworkPort for MockPort {
        async fn inject(
            &self,
            request: PrivilegedNetworkRequest,
        ) -> Result<PrivilegedNetworkSnapshot, PortError> {
            self.inject_calls.fetch_add(1, Ordering::SeqCst);
            if self.fail_inject.load(Ordering::SeqCst) {
                return Err(PortError::PermissionDenied(
                    "mock injection denied".to_owned(),
                ));
            }
            let snapshot = PrivilegedNetworkSnapshot::active(0, request.operations.len());
            *self.snapshot.lock().expect("mock snapshot lock") = snapshot.clone();
            Ok(snapshot)
        }

        async fn cleanup(&self) -> Result<PrivilegedNetworkSnapshot, PortError> {
            self.cleanup_calls.fetch_add(1, Ordering::SeqCst);
            if self.fail_cleanup.load(Ordering::SeqCst) {
                return Err(PortError::PermissionDenied(
                    "mock cleanup denied".to_owned(),
                ));
            }
            let operation_count = self
                .snapshot
                .lock()
                .expect("mock snapshot lock")
                .operation_count;
            let snapshot = PrivilegedNetworkSnapshot::cleaned(0, operation_count, false);
            *self.snapshot.lock().expect("mock snapshot lock") = snapshot.clone();
            Ok(snapshot)
        }

        async fn snapshot(&self) -> Result<PrivilegedNetworkSnapshot, PortError> {
            Ok(self.snapshot.lock().expect("mock snapshot lock").clone())
        }
    }

    fn request() -> PrivilegedNetworkRequest {
        PrivilegedNetworkRequest {
            operations: vec![PrivilegedNetworkOperation::TunService],
        }
    }

    fn mock() -> Arc<MockPort> {
        Arc::new(MockPort {
            snapshot: std::sync::Mutex::new(PrivilegedNetworkSnapshot::default()),
            inject_calls: AtomicUsize::new(0),
            cleanup_calls: AtomicUsize::new(0),
            fail_inject: AtomicBool::new(false),
            fail_cleanup: AtomicBool::new(false),
        })
    }

    #[tokio::test]
    async fn successful_injection_always_cleans_and_reads_back() {
        let port = mock();
        let app = PrivilegedNetworkApplication::new(port.clone());
        let snapshot = app.run(request()).await.expect("mock transaction");
        assert!(snapshot.is_clean());
        assert_eq!(port.inject_calls.load(Ordering::SeqCst), 1);
        assert_eq!(port.cleanup_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn injection_failure_attempts_rollback_and_preserves_primary_error() {
        let port = mock();
        port.fail_inject.store(true, Ordering::SeqCst);
        let app = PrivilegedNetworkApplication::new(port.clone());
        let failure = app.run(request()).await.expect_err("injection must fail");
        assert_eq!(failure.code, ErrorCode::Permission);
        assert_eq!(port.cleanup_calls.load(Ordering::SeqCst), 1);
        assert!(app.snapshot().await.is_clean());
    }

    #[tokio::test]
    async fn cleanup_failure_becomes_a_typed_failed_snapshot() {
        let port = mock();
        port.fail_cleanup.store(true, Ordering::SeqCst);
        let app = PrivilegedNetworkApplication::new(port);
        let failure = app.run(request()).await.expect_err("cleanup must fail");
        assert_eq!(failure.code, ErrorCode::Permission);
        assert!(matches!(
            app.snapshot().await.state,
            PrivilegedNetworkState::Failed { .. }
        ));
    }
}
