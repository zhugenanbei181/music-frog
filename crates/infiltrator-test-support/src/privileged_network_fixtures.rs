//! Isolated privileged-network port fixture.
//!
//! The fixture publishes the real transaction state of one privileged
//! operation: an injection that succeeded, a cleanup that rolled the host
//! back, and a refusal that reports the typed failure instead of a clean host.

use async_trait::async_trait;
use infiltrator_contract::privileged_network::{
    PrivilegedNetworkRequest, PrivilegedNetworkSnapshot,
};
use infiltrator_ports::privileged_network::PrivilegedNetworkPort;
use lurker_core_ports::error::PortError;
use std::sync::Mutex;

/// An isolated privileged-network port that answers with real transaction facts.
pub struct PrivilegedNetworkCapturePort {
    snapshot: Mutex<PrivilegedNetworkSnapshot>,
    revision: Mutex<u64>,
    inject_failure: Mutex<Option<String>>,
    cleanup_failure: Mutex<Option<String>>,
}

impl Default for PrivilegedNetworkCapturePort {
    fn default() -> Self {
        Self::new()
    }
}

impl PrivilegedNetworkCapturePort {
    pub fn new() -> Self {
        Self {
            snapshot: Mutex::new(PrivilegedNetworkSnapshot::default()),
            revision: Mutex::new(1),
            inject_failure: Mutex::new(None),
            cleanup_failure: Mutex::new(None),
        }
    }

    /// Makes the next injection fail with the given reason.
    pub fn fail_injection(&self, reason: impl Into<String>) {
        *self
            .inject_failure
            .lock()
            .expect("privileged capture inject failure") = Some(reason.into());
    }

    /// Makes the next cleanup fail with the given reason.
    pub fn fail_cleanup(&self, reason: impl Into<String>) {
        *self
            .cleanup_failure
            .lock()
            .expect("privileged capture cleanup failure") = Some(reason.into());
    }

    /// The transaction state the port currently publishes.
    pub fn current_snapshot(&self) -> PrivilegedNetworkSnapshot {
        self.snapshot
            .lock()
            .expect("privileged capture snapshot")
            .clone()
    }

    fn next_revision(&self) -> u64 {
        let mut revision = self.revision.lock().expect("privileged capture revision");
        *revision += 1;
        *revision
    }

    fn store(&self, snapshot: PrivilegedNetworkSnapshot) -> PrivilegedNetworkSnapshot {
        let mut stored = self.snapshot.lock().expect("privileged capture snapshot");
        *stored = snapshot.clone();
        snapshot
    }
}

#[async_trait]
impl PrivilegedNetworkPort for PrivilegedNetworkCapturePort {
    async fn inject(
        &self,
        request: PrivilegedNetworkRequest,
    ) -> Result<PrivilegedNetworkSnapshot, PortError> {
        if let Some(reason) = self
            .inject_failure
            .lock()
            .expect("privileged capture inject failure")
            .clone()
        {
            return Err(PortError::PermissionDenied(reason));
        }
        let revision = self.next_revision();
        Ok(self.store(PrivilegedNetworkSnapshot::active(
            revision,
            request.operations.len(),
        )))
    }

    async fn cleanup(&self) -> Result<PrivilegedNetworkSnapshot, PortError> {
        if let Some(reason) = self
            .cleanup_failure
            .lock()
            .expect("privileged capture cleanup failure")
            .clone()
        {
            return Err(PortError::PermissionDenied(reason));
        }
        let revision = self.next_revision();
        let operation_count = self.current_snapshot().operation_count;
        Ok(self.store(PrivilegedNetworkSnapshot::cleaned(
            revision,
            operation_count,
            true,
        )))
    }

    async fn snapshot(&self) -> Result<PrivilegedNetworkSnapshot, PortError> {
        Ok(self.current_snapshot())
    }
}
