//! Host-owned system HTTP/SOCKS proxy capability.

use async_trait::async_trait;
use futures_util::lock;
use infiltrator_contract::system_proxy::{
    SystemProxyClaim, SystemProxyClaimState, SystemProxyDesiredState, SystemProxyObservation,
    SystemProxyRecoveryReport, SystemProxyRecoverySnapshot, SystemProxyReleaseReport,
};
use lurker_core_contract::capability::Capability;
use lurker_core_ports::error::PortError;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex, OnceLock};

static SHARED_SYSTEM_PROXY_OPERATION: OnceLock<Arc<lock::Mutex<()>>> = OnceLock::new();
static SHARED_SYSTEM_PROXY_GENERATION: OnceLock<Arc<AtomicU64>> = OnceLock::new();

#[async_trait]
pub trait SystemProxyPort: Send + Sync {
    /// Claim-capable ports return the owner-wide claim cell shared by all handles.
    fn shared_claim(&self) -> Arc<Mutex<SystemProxyClaimState>> {
        Arc::new(Mutex::new(SystemProxyClaimState::Unclaimed))
    }

    /// Serializes application operations for the process-wide system proxy.
    fn shared_operation(&self) -> Arc<lock::Mutex<()>> {
        SHARED_SYSTEM_PROXY_OPERATION
            .get_or_init(|| Arc::new(lock::Mutex::new(())))
            .clone()
    }

    /// Invalidates cached snapshots after any owner-mediated state transition.
    fn shared_snapshot_generation(&self) -> Arc<AtomicU64> {
        SHARED_SYSTEM_PROXY_GENERATION
            .get_or_init(|| Arc::new(AtomicU64::new(0)))
            .clone()
    }

    fn shared_recovery(&self) -> Arc<Mutex<SystemProxyRecoverySnapshot>> {
        Arc::new(Mutex::new(SystemProxyRecoverySnapshot::default()))
    }

    /// Atomically capture the previous value, persist a claim, publish it to
    /// `shared_claim`, and apply it before returning.
    async fn claim_and_apply(
        &self,
        desired: SystemProxyDesiredState,
    ) -> Result<(SystemProxyClaim, SystemProxyObservation), PortError> {
        let _ = desired;
        Err(PortError::unsupported(
            Capability::SystemProxy,
            "host adapter does not implement owned system-proxy claims",
        ))
    }

    /// Resolve a stale journal and clear only its matching shared claim before
    /// returning a completed recovery report.
    async fn recover_orphaned(&self) -> Result<SystemProxyRecoveryReport, PortError> {
        Ok(SystemProxyRecoveryReport::NotNeeded)
    }

    /// Release only the exact claim observed by the caller. Adapters publish
    /// each claim transition to `shared_claim` before returning. An already-
    /// observed external change must remain untouched; cooperating adapters
    /// serialize with one another, but cannot serialize non-cooperating OS writers.
    async fn release_owned(
        &self,
        expected: &SystemProxyClaim,
    ) -> Result<SystemProxyReleaseReport, PortError> {
        let _ = expected;
        Err(PortError::unsupported(
            Capability::SystemProxy,
            "host adapter does not implement ownership-safe release",
        ))
    }

    async fn snapshot(&self) -> Result<SystemProxyObservation, PortError>;
}
