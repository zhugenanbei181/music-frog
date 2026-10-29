//! System HTTP/SOCKS proxy use-case over a host-owned port.

use futures_util::lock;
use infiltrator_contract::system_proxy::{
    SystemProxyClaim, SystemProxyClaimState, SystemProxyDesiredState, SystemProxyObservation,
    SystemProxyRecoveryReport, SystemProxyRecoverySnapshot, SystemProxyRecoveryStatus,
    SystemProxyReleaseReport, SystemProxySnapshot,
};
use infiltrator_ports::system_proxy::SystemProxyPort;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_ports::error::PortError;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const SYSTEM_PROXY_SNAPSHOT_CACHE_TTL: Duration = Duration::from_secs(3);

#[derive(Clone)]
pub struct SystemProxyApplication {
    port: Arc<dyn SystemProxyPort>,
    next_revision: Arc<AtomicU64>,
    last_snapshot: Arc<Mutex<Option<(Instant, u64, SystemProxySnapshot)>>>,
    claim: Arc<Mutex<SystemProxyClaimState>>,
    repair_count: Arc<AtomicU64>,
    recovery: Arc<Mutex<SystemProxyRecoverySnapshot>>,
    operation: Arc<lock::Mutex<()>>,
    snapshot_generation: Arc<AtomicU64>,
}

impl SystemProxyApplication {
    pub fn new(port: Arc<dyn SystemProxyPort>) -> Self {
        let claim = port.shared_claim();
        let recovery = port.shared_recovery();
        let operation = port.shared_operation();
        let snapshot_generation = port.shared_snapshot_generation();
        Self {
            port,
            next_revision: Arc::new(AtomicU64::new(1)),
            last_snapshot: Arc::new(Mutex::new(None)),
            claim,
            repair_count: Arc::new(AtomicU64::new(0)),
            recovery,
            operation,
            snapshot_generation,
        }
    }

    pub fn identity(&self) -> usize {
        Arc::as_ptr(&self.port) as *const () as usize
    }

    pub fn recovery_snapshot(&self) -> SystemProxyRecoverySnapshot {
        self.recovery
            .lock()
            .expect("system proxy recovery lock")
            .clone()
    }

    pub async fn recover_orphaned(&self) -> SystemProxyRecoverySnapshot {
        let _operation = self.operation.lock().await;
        self.advance_snapshot_generation();
        let revision = self.next_revision.fetch_add(1, Ordering::Relaxed);
        let status = match self.port.recover_orphaned().await {
            Ok(SystemProxyRecoveryReport::NotNeeded) => SystemProxyRecoveryStatus::NotNeeded,
            Ok(SystemProxyRecoveryReport::Restored { previous, restored }) => {
                SystemProxyRecoveryStatus::Restored { previous, restored }
            }
            Ok(SystemProxyRecoveryReport::SkippedExternal { expected, observed }) => {
                SystemProxyRecoveryStatus::SkippedExternal { expected, observed }
            }
            Ok(SystemProxyRecoveryReport::SkippedLiveOwner { owner_pid }) => {
                SystemProxyRecoveryStatus::SkippedLiveOwner { owner_pid }
            }
            Err(error) => SystemProxyRecoveryStatus::Failed {
                failure: Failure::from(error),
            },
        };
        if status == SystemProxyRecoveryStatus::NotNeeded {
            let previous = self.recovery_snapshot();
            if !matches!(
                previous.status,
                SystemProxyRecoveryStatus::Unknown | SystemProxyRecoveryStatus::NotNeeded
            ) {
                return previous;
            }
        }
        let snapshot = SystemProxyRecoverySnapshot { status, revision };
        *self.recovery.lock().expect("system proxy recovery lock") = snapshot.clone();
        snapshot
    }

    pub async fn snapshot(&self) -> SystemProxySnapshot {
        let _operation = self.operation.lock().await;
        self.snapshot_and_cache().await
    }

    pub async fn snapshot_cached(&self) -> SystemProxySnapshot {
        let _operation = self.operation.lock().await;
        if let Some((observed_at, generation, snapshot)) = self
            .last_snapshot
            .lock()
            .expect("system proxy snapshot cache lock")
            .as_ref()
            .cloned()
            && generation == self.snapshot_generation.load(Ordering::Acquire)
            && observed_at.elapsed() < SYSTEM_PROXY_SNAPSHOT_CACHE_TTL
        {
            return snapshot;
        }
        self.snapshot_and_cache().await
    }

    async fn snapshot_and_cache(&self) -> SystemProxySnapshot {
        let snapshot = self.reconcile_fresh().await;
        self.cache(snapshot.clone());
        snapshot
    }

    /// Apply and read back the host proxy state. A successful OS command that
    /// leaves the requested state absent is still a failure, never a success.
    pub async fn set_enabled(
        &self,
        enabled: bool,
        endpoint: Option<String>,
        bypass: Option<String>,
    ) -> Result<SystemProxySnapshot, Failure> {
        let _operation = self.operation.lock().await;
        if enabled
            && endpoint
                .as_deref()
                .is_none_or(|value| value.trim().is_empty())
        {
            return Err(Failure::new(
                ErrorCode::NotReady,
                "system proxy cannot be enabled without a controller endpoint",
                true,
            ));
        }
        self.advance_snapshot_generation();
        if !enabled {
            let revision = self.next_revision.fetch_add(1, Ordering::Relaxed);
            let report = self.release_current_claim().await?;
            let snapshot = match report {
                SystemProxyReleaseReport::SkippedLiveOwner { current, .. } => {
                    return Err(Failure::new(
                        ErrorCode::InvalidState,
                        format!(
                            "system proxy claim is owned by live process {}",
                            current.owner_pid
                        ),
                        true,
                    ));
                }
                SystemProxyReleaseReport::SkippedStaleClaim { current, .. } => {
                    return Err(Failure::new(
                        ErrorCode::InvalidState,
                        format!(
                            "system proxy claim changed to revision {}; retry with the current claim",
                            current.revision
                        ),
                        true,
                    ));
                }
                report => self.snapshot_after_release(revision, report),
            };
            self.cache(snapshot.clone());
            return Ok(snapshot);
        }

        let desired = SystemProxyDesiredState {
            enabled: true,
            endpoint,
            bypass,
        };
        let (claim, observation) = self
            .port
            .claim_and_apply(desired)
            .await
            .map_err(Failure::from)?;
        if claim.observed != observation {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                format!(
                    "system proxy readback mismatch: claim {:?}, observed {:?}",
                    claim.desired, observation
                ),
                true,
            ));
        }
        let shared_claim = self.claim.lock().expect("system proxy claim lock").clone();
        if shared_claim != SystemProxyClaimState::Claimed(claim) {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "system proxy claim was not published by the host owner",
                true,
            ));
        }
        let snapshot = SystemProxySnapshot::from_observation(
            self.next_revision.fetch_add(1, Ordering::Relaxed),
            observation,
        )
        .with_owned()
        .with_repair_count(self.repair_count.load(Ordering::Relaxed));
        self.cache(snapshot.clone());
        Ok(snapshot)
    }

    async fn release_current_claim(&self) -> Result<SystemProxyReleaseReport, Failure> {
        let claim = { self.claim.lock().expect("system proxy claim lock").clone() };
        match claim {
            SystemProxyClaimState::Unclaimed => self
                .port
                .snapshot()
                .await
                .map(|observed| SystemProxyReleaseReport::NoClaim { observed })
                .map_err(Failure::from),
            SystemProxyClaimState::Claimed(claim) => self.release_expected_claim(&claim).await,
        }
    }

    async fn release_expected_claim(
        &self,
        expected: &SystemProxyClaim,
    ) -> Result<SystemProxyReleaseReport, Failure> {
        let report = self
            .port
            .release_owned(expected)
            .await
            .map_err(Failure::from)?;
        match report {
            SystemProxyReleaseReport::NoClaim { .. } => Err(Failure::new(
                ErrorCode::InvalidState,
                "system proxy claim exists locally but is missing from the host owner",
                true,
            )),
            report => Ok(report),
        }
    }

    async fn reconcile_fresh(&self) -> SystemProxySnapshot {
        let revision = self.next_revision.fetch_add(1, Ordering::Relaxed);
        let observation = match self.port.snapshot().await {
            Ok(observation) => observation,
            Err(PortError::Unsupported { reason, .. }) => {
                return SystemProxySnapshot::unsupported(revision, reason);
            }
            Err(error) => return SystemProxySnapshot::failed(revision, Failure::from(error)),
        };
        let claim = self.claim.lock().expect("system proxy claim lock").clone();
        let SystemProxyClaimState::Claimed(claim) = claim else {
            return SystemProxySnapshot::from_observation(revision, observation);
        };
        let repair_count = self.repair_count.load(Ordering::Relaxed);
        if claim.observed == observation {
            return SystemProxySnapshot::from_observation(revision, observation)
                .with_owned()
                .with_repair_count(repair_count);
        }
        self.advance_snapshot_generation();
        let report = match self.release_expected_claim(&claim).await {
            Ok(report) => report,
            Err(failure) => return SystemProxySnapshot::failed(revision, failure),
        };
        self.snapshot_after_release(revision, report)
    }

    fn snapshot_after_release(
        &self,
        revision: u64,
        report: SystemProxyReleaseReport,
    ) -> SystemProxySnapshot {
        match report {
            SystemProxyReleaseReport::NoClaim { observed }
            | SystemProxyReleaseReport::SkippedExternal { observed, .. } => {
                self.snapshot_for_current_claim(revision, observed)
            }
            SystemProxyReleaseReport::Restored { restored, .. } => {
                self.snapshot_for_current_claim(revision, restored)
            }
            SystemProxyReleaseReport::SkippedStaleClaim { current, observed } => {
                self.snapshot_for_claim_readback(revision, current, observed)
            }
            SystemProxyReleaseReport::SkippedLiveOwner { current, observed } => {
                self.snapshot_for_claim_readback(revision, current, observed)
            }
        }
    }

    fn snapshot_for_claim_readback(
        &self,
        revision: u64,
        current: SystemProxyClaim,
        observed: SystemProxyObservation,
    ) -> SystemProxySnapshot {
        let shared_claim = self.claim.lock().expect("system proxy claim lock").clone();
        if current.observed == observed
            && shared_claim == SystemProxyClaimState::Claimed(current.clone())
        {
            return SystemProxySnapshot::from_observation(revision, observed)
                .with_owned()
                .with_repair_count(self.repair_count.load(Ordering::Relaxed));
        }
        SystemProxySnapshot::failed(
            revision,
            Failure::new(
                ErrorCode::InvalidState,
                format!(
                    "system proxy claim revision {} does not match host readback",
                    current.revision
                ),
                true,
            ),
        )
    }

    fn snapshot_for_current_claim(
        &self,
        revision: u64,
        observed: SystemProxyObservation,
    ) -> SystemProxySnapshot {
        let claim = self.claim.lock().expect("system proxy claim lock").clone();
        match claim {
            SystemProxyClaimState::Unclaimed => {
                SystemProxySnapshot::from_observation(revision, observed)
                    .with_repair_count(self.repair_count.load(Ordering::Relaxed))
            }
            SystemProxyClaimState::Claimed(claim) => {
                self.snapshot_for_claim_readback(revision, claim, observed)
            }
        }
    }

    fn cache(&self, snapshot: SystemProxySnapshot) {
        let generation = self.snapshot_generation.load(Ordering::Acquire);
        *self
            .last_snapshot
            .lock()
            .expect("system proxy snapshot cache lock") =
            Some((Instant::now(), generation, snapshot));
    }

    fn advance_snapshot_generation(&self) {
        self.snapshot_generation.fetch_add(1, Ordering::AcqRel);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use infiltrator_contract::system_proxy::{
        SystemProxyClaim, SystemProxyClaimState, SystemProxyOwnership, SystemProxyStatus,
    };
    use std::process::id;
    use std::sync::atomic::AtomicUsize;

    struct TestPort {
        observation: Mutex<SystemProxyObservation>,
        apply_calls: AtomicUsize,
        snapshot_calls: AtomicUsize,
        claim: Arc<Mutex<SystemProxyClaimState>>,
        operation: Arc<lock::Mutex<()>>,
        snapshot_generation: Arc<AtomicU64>,
        recovery_report: Mutex<Option<SystemProxyRecoveryReport>>,
        release_override: Mutex<Option<SystemProxyReleaseReport>>,
    }

    impl TestPort {
        fn new(observation: SystemProxyObservation) -> Self {
            Self {
                observation: Mutex::new(observation),
                apply_calls: AtomicUsize::new(0),
                snapshot_calls: AtomicUsize::new(0),
                claim: Arc::new(Mutex::new(SystemProxyClaimState::Unclaimed)),
                operation: Arc::new(lock::Mutex::new(())),
                snapshot_generation: Arc::new(AtomicU64::new(0)),
                recovery_report: Mutex::new(None),
                release_override: Mutex::new(None),
            }
        }
    }

    #[async_trait]
    impl SystemProxyPort for TestPort {
        fn shared_claim(&self) -> Arc<Mutex<SystemProxyClaimState>> {
            self.claim.clone()
        }

        fn shared_operation(&self) -> Arc<lock::Mutex<()>> {
            self.operation.clone()
        }

        fn shared_snapshot_generation(&self) -> Arc<AtomicU64> {
            self.snapshot_generation.clone()
        }

        async fn claim_and_apply(
            &self,
            desired: SystemProxyDesiredState,
        ) -> Result<(SystemProxyClaim, SystemProxyObservation), PortError> {
            let mut state = self.claim.lock().expect("claim lock");
            let previous = self.observation.lock().expect("proxy lock").clone();
            let revision = match &*state {
                SystemProxyClaimState::Unclaimed => 1,
                SystemProxyClaimState::Claimed(claim) => claim.revision.saturating_add(1),
            };
            let observed = SystemProxyObservation {
                enabled: desired.enabled,
                endpoint: desired.endpoint.clone(),
                bypass: desired.bypass.clone(),
            };
            *self.observation.lock().expect("proxy lock") = observed.clone();
            self.apply_calls.fetch_add(1, Ordering::SeqCst);
            let claim = SystemProxyClaim {
                revision,
                owner_pid: id(),
                owner_start_time: 1,
                previous,
                desired,
                observed: observed.clone(),
            };
            *state = SystemProxyClaimState::Claimed(claim.clone());
            Ok((claim, observed))
        }

        async fn release_owned(
            &self,
            expected: &SystemProxyClaim,
        ) -> Result<SystemProxyReleaseReport, PortError> {
            let observed = self.observation.lock().expect("proxy lock").clone();
            if let Some(report) = self
                .release_override
                .lock()
                .expect("release override lock")
                .take()
            {
                if let SystemProxyReleaseReport::SkippedStaleClaim { current, .. }
                | SystemProxyReleaseReport::SkippedLiveOwner { current, .. } = &report
                {
                    *self.claim.lock().expect("claim lock") =
                        SystemProxyClaimState::Claimed(current.clone());
                }
                return Ok(report);
            }
            let mut state = self.claim.lock().expect("claim lock");
            let current = match &*state {
                SystemProxyClaimState::Unclaimed => {
                    return Ok(SystemProxyReleaseReport::NoClaim { observed });
                }
                SystemProxyClaimState::Claimed(claim) => claim.clone(),
            };
            if current != *expected {
                return Ok(SystemProxyReleaseReport::SkippedStaleClaim { current, observed });
            }
            if current.observed != observed {
                *state = SystemProxyClaimState::Unclaimed;
                return Ok(SystemProxyReleaseReport::SkippedExternal {
                    expected: current,
                    observed,
                });
            }
            let restored = current.previous.clone();
            *self.observation.lock().expect("proxy lock") = restored.clone();
            *state = SystemProxyClaimState::Unclaimed;
            self.apply_calls.fetch_add(1, Ordering::SeqCst);
            Ok(SystemProxyReleaseReport::Restored {
                previous: current.previous,
                restored,
            })
        }

        async fn recover_orphaned(&self) -> Result<SystemProxyRecoveryReport, PortError> {
            Ok(self
                .recovery_report
                .lock()
                .expect("recovery report lock")
                .take()
                .unwrap_or(SystemProxyRecoveryReport::NotNeeded))
        }

        async fn snapshot(&self) -> Result<SystemProxyObservation, PortError> {
            self.snapshot_calls.fetch_add(1, Ordering::SeqCst);
            Ok(self.observation.lock().expect("proxy lock").clone())
        }
    }

    fn third_party_proxy() -> SystemProxyObservation {
        SystemProxyObservation {
            enabled: true,
            endpoint: Some("proxy.example:8080".to_owned()),
            bypass: Some("localhost".to_owned()),
        }
    }

    #[tokio::test]
    async fn separate_compositions_share_the_same_operation_lock() {
        let port = Arc::new(TestPort::new(SystemProxyObservation::default()));
        let command_application = SystemProxyApplication::new(port.clone());
        let surface_application = SystemProxyApplication::new(port);

        assert!(Arc::ptr_eq(
            &command_application.operation,
            &surface_application.operation
        ));
    }

    #[tokio::test]
    async fn application_claims_and_reads_back_owned_proxy_state() {
        let port = Arc::new(TestPort::new(SystemProxyObservation::default()));
        let application = SystemProxyApplication::new(port.clone());
        let snapshot = application
            .set_enabled(true, Some("127.0.0.1:7890".to_owned()), None)
            .await
            .expect("claim proxy");

        assert_eq!(snapshot.status, SystemProxyStatus::Enabled);
        assert_eq!(snapshot.ownership, SystemProxyOwnership::Owned);
        assert_eq!(port.apply_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn separate_compositions_share_the_same_claim_readback() {
        let port = Arc::new(TestPort::new(SystemProxyObservation::default()));
        let command_application = SystemProxyApplication::new(port.clone());
        let surface_application = SystemProxyApplication::new(port.clone());
        assert!(Arc::ptr_eq(
            &command_application.operation,
            &surface_application.operation
        ));
        command_application
            .set_enabled(true, Some("127.0.0.1:7890".to_owned()), None)
            .await
            .expect("claim proxy");

        let snapshot = surface_application.snapshot().await;
        assert_eq!(snapshot.ownership, SystemProxyOwnership::Owned);
        assert_eq!(port.apply_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn disabling_without_a_claim_preserves_an_external_proxy() {
        let external = third_party_proxy();
        let port = Arc::new(TestPort::new(external.clone()));
        let application = SystemProxyApplication::new(port.clone());
        let snapshot = application
            .set_enabled(false, None, None)
            .await
            .expect("safe no-claim release");

        assert_eq!(snapshot.status, SystemProxyStatus::Enabled);
        assert_eq!(snapshot.ownership, SystemProxyOwnership::Unmanaged);
        assert_eq!(snapshot.endpoint.as_deref(), Some("proxy.example:8080"));
        assert_eq!(port.apply_calls.load(Ordering::SeqCst), 0);
        assert_eq!(*port.observation.lock().expect("proxy lock"), external);
    }

    #[tokio::test]
    async fn disabling_restores_the_value_that_preceded_our_claim() {
        let external = third_party_proxy();
        let port = Arc::new(TestPort::new(external.clone()));
        let application = SystemProxyApplication::new(port.clone());
        application
            .set_enabled(true, Some("127.0.0.1:7890".to_owned()), None)
            .await
            .expect("claim proxy");

        let snapshot = application
            .set_enabled(false, None, None)
            .await
            .expect("release own claim");
        assert_eq!(snapshot.status, SystemProxyStatus::Enabled);
        assert_eq!(snapshot.ownership, SystemProxyOwnership::Unmanaged);
        assert_eq!(snapshot.endpoint.as_deref(), Some("proxy.example:8080"));
        assert_eq!(*port.observation.lock().expect("proxy lock"), external);
        assert_eq!(
            *port.claim.lock().expect("claim lock"),
            SystemProxyClaimState::Unclaimed
        );
    }

    #[tokio::test]
    async fn disabling_fails_closed_when_the_host_owner_has_no_matching_claim() {
        let port = Arc::new(TestPort::new(SystemProxyObservation::default()));
        let application = SystemProxyApplication::new(port.clone());
        application
            .set_enabled(true, Some("127.0.0.1:7890".to_owned()), None)
            .await
            .expect("claim proxy");
        let observed = port.observation.lock().expect("proxy lock").clone();
        *port.release_override.lock().expect("release override lock") =
            Some(SystemProxyReleaseReport::NoClaim {
                observed: observed.clone(),
            });

        let failure = application
            .set_enabled(false, None, None)
            .await
            .expect_err("a missing host claim must not report a successful disable");

        assert_eq!(failure.code, ErrorCode::InvalidState);
        assert!(failure.retryable);
        assert_eq!(*port.observation.lock().expect("proxy lock"), observed);
        assert!(matches!(
            *port.claim.lock().expect("claim lock"),
            SystemProxyClaimState::Claimed(_)
        ));
        assert_eq!(port.apply_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn disabling_with_a_stale_claim_fails_retryably_and_preserves_the_current_claim() {
        let port = Arc::new(TestPort::new(SystemProxyObservation::default()));
        let application = SystemProxyApplication::new(port.clone());
        application
            .set_enabled(true, Some("127.0.0.1:7890".to_owned()), None)
            .await
            .expect("claim proxy");
        let (desired, current_observation) = {
            let state = port.claim.lock().expect("claim lock");
            let SystemProxyClaimState::Claimed(claim) = &*state else {
                panic!("claim must be published")
            };
            (claim.desired.clone(), claim.observed.clone())
        };
        let current = SystemProxyClaim {
            revision: 2,
            owner_pid: id(),
            owner_start_time: 1,
            previous: SystemProxyObservation::default(),
            desired,
            observed: current_observation.clone(),
        };
        *port.release_override.lock().expect("release override lock") =
            Some(SystemProxyReleaseReport::SkippedStaleClaim {
                current: current.clone(),
                observed: current_observation,
            });

        let failure = application
            .set_enabled(false, None, None)
            .await
            .expect_err("stale claim must not report a successful disable");

        assert_eq!(failure.code, ErrorCode::InvalidState);
        assert!(failure.retryable);
        assert_eq!(
            *port.claim.lock().expect("claim lock"),
            SystemProxyClaimState::Claimed(current)
        );
        assert_eq!(port.apply_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn snapshot_reconciles_a_new_claim_from_authoritative_release_readback() {
        let port = Arc::new(TestPort::new(SystemProxyObservation::default()));
        let application = SystemProxyApplication::new(port.clone());
        application
            .set_enabled(true, Some("127.0.0.1:7890".to_owned()), None)
            .await
            .expect("initial claim");
        let mut current = match &*port.claim.lock().expect("claim lock") {
            SystemProxyClaimState::Claimed(claim) => claim.clone(),
            SystemProxyClaimState::Unclaimed => panic!("claim must be published"),
        };
        current.revision += 1;
        let current_observation = SystemProxyObservation {
            enabled: true,
            endpoint: Some("127.0.0.1:7891".to_owned()),
            bypass: None,
        };
        current.desired.endpoint = current_observation.endpoint.clone();
        current.observed = current_observation.clone();
        *port.observation.lock().expect("proxy lock") = current_observation.clone();
        *port.release_override.lock().expect("release override lock") =
            Some(SystemProxyReleaseReport::SkippedStaleClaim {
                current: current.clone(),
                observed: current_observation,
            });

        let snapshot = application.snapshot().await;

        assert_eq!(snapshot.status, SystemProxyStatus::Enabled);
        assert_eq!(snapshot.ownership, SystemProxyOwnership::Owned);
        assert_eq!(snapshot.endpoint.as_deref(), Some("127.0.0.1:7891"));
        assert_eq!(
            *port.claim.lock().expect("claim lock"),
            SystemProxyClaimState::Claimed(current)
        );
    }

    #[tokio::test]
    async fn snapshot_fails_closed_when_release_readback_has_no_matching_claim() {
        let port = Arc::new(TestPort::new(SystemProxyObservation::default()));
        let application = SystemProxyApplication::new(port.clone());
        application
            .set_enabled(true, Some("127.0.0.1:7890".to_owned()), None)
            .await
            .expect("claim proxy");
        let external = third_party_proxy();
        *port.observation.lock().expect("proxy lock") = external.clone();
        *port.release_override.lock().expect("release override lock") =
            Some(SystemProxyReleaseReport::NoClaim {
                observed: external.clone(),
            });

        let snapshot = application.snapshot().await;

        match snapshot.status {
            SystemProxyStatus::Failed { failure } => {
                assert_eq!(failure.code, ErrorCode::InvalidState);
                assert!(failure.retryable);
            }
            status => panic!("expected fail-closed snapshot, got {status:?}"),
        }
        assert_eq!(snapshot.ownership, SystemProxyOwnership::Unknown);
        assert_eq!(*port.observation.lock().expect("proxy lock"), external);
        assert!(matches!(
            *port.claim.lock().expect("claim lock"),
            SystemProxyClaimState::Claimed(_)
        ));
        assert_eq!(port.apply_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn disabling_after_external_change_preserves_the_external_value() {
        let port = Arc::new(TestPort::new(SystemProxyObservation::default()));
        let application = SystemProxyApplication::new(port.clone());
        application
            .set_enabled(true, Some("127.0.0.1:7890".to_owned()), None)
            .await
            .expect("claim proxy");
        let external = third_party_proxy();
        *port.observation.lock().expect("proxy lock") = external.clone();

        let snapshot = application
            .set_enabled(false, None, None)
            .await
            .expect("release stale claim safely");
        assert_eq!(snapshot.status, SystemProxyStatus::Enabled);
        assert_eq!(snapshot.ownership, SystemProxyOwnership::Unmanaged);
        assert_eq!(*port.observation.lock().expect("proxy lock"), external);
        assert_eq!(
            *port.claim.lock().expect("claim lock"),
            SystemProxyClaimState::Unclaimed
        );
    }

    #[tokio::test]
    async fn observation_of_external_override_relinquishes_claim_without_repairing_it() {
        let port = Arc::new(TestPort::new(SystemProxyObservation::default()));
        let application = SystemProxyApplication::new(port.clone());
        application
            .set_enabled(true, Some("127.0.0.1:7890".to_owned()), None)
            .await
            .expect("claim proxy");
        let external = third_party_proxy();
        *port.observation.lock().expect("proxy lock") = external.clone();

        let snapshot = application.snapshot().await;
        assert_eq!(snapshot.status, SystemProxyStatus::Enabled);
        assert_eq!(snapshot.ownership, SystemProxyOwnership::Unmanaged);
        assert_eq!(snapshot.endpoint.as_deref(), Some("proxy.example:8080"));
        assert_eq!(port.apply_calls.load(Ordering::SeqCst), 1);
        assert_eq!(*port.observation.lock().expect("proxy lock"), external);
        assert_eq!(
            *port.claim.lock().expect("claim lock"),
            SystemProxyClaimState::Unclaimed
        );
    }

    #[tokio::test]
    async fn application_caches_unowned_surface_observations() {
        let port = Arc::new(TestPort::new(SystemProxyObservation::default()));
        let application = SystemProxyApplication::new(port.clone());
        let first = application.snapshot_cached().await;
        let second = application.snapshot_cached().await;
        assert_eq!(first, second);
        assert_eq!(port.apply_calls.load(Ordering::SeqCst), 0);
        assert_eq!(port.snapshot_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn separate_composition_claim_invalidates_cached_surface_snapshot() {
        let port = Arc::new(TestPort::new(SystemProxyObservation::default()));
        let command_application = SystemProxyApplication::new(port.clone());
        let surface_application = SystemProxyApplication::new(port.clone());
        let cached = surface_application.snapshot_cached().await;
        assert_eq!(cached.ownership, SystemProxyOwnership::Unmanaged);
        assert_eq!(port.snapshot_calls.load(Ordering::SeqCst), 1);

        command_application
            .set_enabled(true, Some("127.0.0.1:7890".to_owned()), None)
            .await
            .expect("claim proxy from command composition");

        let refreshed = surface_application.snapshot_cached().await;
        assert_eq!(refreshed.ownership, SystemProxyOwnership::Owned);
        assert_eq!(refreshed.endpoint.as_deref(), Some("127.0.0.1:7890"));
        assert_eq!(port.snapshot_calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn application_publishes_typed_startup_recovery_result() {
        let port = Arc::new(TestPort::new(SystemProxyObservation::default()));
        *port.recovery_report.lock().expect("recovery report lock") =
            Some(SystemProxyRecoveryReport::SkippedLiveOwner { owner_pid: 4242 });
        let application = SystemProxyApplication::new(port);
        let snapshot = application.recover_orphaned().await;
        assert!(matches!(
            snapshot.status,
            SystemProxyRecoveryStatus::SkippedLiveOwner { owner_pid: 4242 }
        ));
        assert_eq!(application.recovery_snapshot(), snapshot);
    }
}
