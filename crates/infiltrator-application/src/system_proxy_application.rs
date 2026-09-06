//! System HTTP/SOCKS proxy use-case over a host-owned port.

use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::system_proxy::{
    SystemProxyDesiredState, SystemProxyObservation, SystemProxyRecoveryReport,
    SystemProxyRecoverySnapshot, SystemProxyRecoveryStatus, SystemProxySnapshot,
};
use infiltrator_ports::error::PortError;
use infiltrator_ports::system_proxy::SystemProxyPort;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const SYSTEM_PROXY_SNAPSHOT_CACHE_TTL: Duration = Duration::from_secs(3);

#[derive(Clone)]
pub struct SystemProxyApplication {
    port: Arc<dyn SystemProxyPort>,
    next_revision: Arc<AtomicU64>,
    last_snapshot: Arc<Mutex<Option<(Instant, SystemProxySnapshot)>>>,
    desired: Arc<Mutex<Option<SystemProxyDesiredState>>>,
    repair_count: Arc<AtomicU64>,
    recovery: Arc<Mutex<SystemProxyRecoverySnapshot>>,
}

impl SystemProxyApplication {
    pub fn new(port: Arc<dyn SystemProxyPort>) -> Self {
        let desired = port.shared_target();
        let recovery = port.shared_recovery();
        Self {
            port,
            next_revision: Arc::new(AtomicU64::new(1)),
            last_snapshot: Arc::new(Mutex::new(None)),
            desired,
            repair_count: Arc::new(AtomicU64::new(0)),
            recovery,
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

    pub async fn clear_recovery(&self) -> Result<(), Failure> {
        self.port.clear_recovery().await.map_err(Failure::from)
    }

    pub async fn snapshot(&self) -> SystemProxySnapshot {
        let snapshot = self.reconcile_fresh().await;
        self.cache(snapshot.clone());
        snapshot
    }

    pub async fn snapshot_cached(&self) -> SystemProxySnapshot {
        if let Some((observed_at, snapshot)) = self
            .last_snapshot
            .lock()
            .expect("system proxy snapshot cache lock")
            .as_ref()
            .cloned()
            && observed_at.elapsed() < SYSTEM_PROXY_SNAPSHOT_CACHE_TTL
        {
            return snapshot;
        }
        self.snapshot().await
    }

    /// Apply and read back the host proxy state. A successful OS command that
    /// leaves the requested state absent is still a failure, never a success.
    pub async fn set_enabled(
        &self,
        enabled: bool,
        endpoint: Option<String>,
        bypass: Option<String>,
    ) -> Result<SystemProxySnapshot, Failure> {
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
        let target = SystemProxyDesiredState {
            enabled,
            endpoint: endpoint.clone(),
            bypass: bypass.clone(),
        };

        // Disabling is a clean ownership release. Apply and read back first,
        // then remove the crash-recovery journal; otherwise a crash after a
        // user-initiated disable could incorrectly restore the old proxy on
        // the next startup.
        if !enabled {
            self.port
                .apply(None, None)
                .await
                .map_err(Failure::from)?;
            let observation = self.port.snapshot().await.map_err(Failure::from)?;
            if !target_matches(&target, &observation) {
                return Err(Failure::new(
                    ErrorCode::InvalidState,
                    format!(
                        "system proxy readback mismatch: requested {:?}, observed {:?}",
                        target, observation
                    ),
                    true,
                ));
            }
            self.port.clear_recovery().await.map_err(Failure::from)?;
            *self.desired.lock().expect("system proxy target lock") = None;
            let snapshot = SystemProxySnapshot::from_observation(
                self.next_revision.fetch_add(1, Ordering::Relaxed),
                observation,
            )
            .with_repair_count(self.repair_count.load(Ordering::Relaxed));
            self.cache(snapshot.clone());
            return Ok(snapshot);
        }

        let previous = self.port.snapshot().await.map_err(Failure::from)?;
        self.port
            .arm_recovery(previous, target.clone())
            .await
            .map_err(Failure::from)?;
        self.port
            .apply(endpoint, bypass)
            .await
            .map_err(Failure::from)?;
        let observation = self.port.snapshot().await.map_err(Failure::from)?;
        if !target_matches(&target, &observation) {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                format!(
                    "system proxy readback mismatch: requested {:?}, observed {:?}",
                    target, observation
                ),
                true,
            ));
        }
        *self.desired.lock().expect("system proxy target lock") = Some(target);
        let snapshot = SystemProxySnapshot::from_observation(
            self.next_revision.fetch_add(1, Ordering::Relaxed),
            observation,
        )
        .with_owned()
        .with_repair_count(self.repair_count.load(Ordering::Relaxed));
        self.cache(snapshot.clone());
        Ok(snapshot)
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
        let target = self
            .desired
            .lock()
            .expect("system proxy target lock")
            .clone();
        let Some(target) = target else {
            return SystemProxySnapshot::from_observation(revision, observation);
        };
        let repair_count = self.repair_count.load(Ordering::Relaxed);
        if target_matches(&target, &observation) {
            return SystemProxySnapshot::from_observation(revision, observation)
                .with_owned()
                .with_repair_count(repair_count);
        }

        if let Err(error) = self
            .port
            .apply(target.endpoint.clone(), target.bypass.clone())
            .await
        {
            return SystemProxySnapshot::failed(revision, Failure::from(error));
        }
        let repaired = match self.port.snapshot().await {
            Ok(observation) => observation,
            Err(error) => return SystemProxySnapshot::failed(revision, Failure::from(error)),
        };
        if !target_matches(&target, &repaired) {
            return SystemProxySnapshot::failed(
                revision,
                Failure::new(
                    ErrorCode::InvalidState,
                    format!(
                        "system proxy repair readback mismatch: target {:?}, observed {:?}",
                        target, repaired
                    ),
                    true,
                ),
            );
        }
        let repair_count = self.repair_count.fetch_add(1, Ordering::Relaxed) + 1;
        SystemProxySnapshot::from_observation(revision, repaired).with_repaired(repair_count)
    }

    fn cache(&self, snapshot: SystemProxySnapshot) {
        *self
            .last_snapshot
            .lock()
            .expect("system proxy snapshot cache lock") = Some((Instant::now(), snapshot));
    }
}

fn target_matches(target: &SystemProxyDesiredState, observation: &SystemProxyObservation) -> bool {
    if target.enabled != observation.enabled {
        return false;
    }
    if !target.enabled {
        return true;
    }
    target.endpoint == observation.endpoint
        && target
            .bypass
            .as_ref()
            .is_none_or(|bypass| observation.bypass.as_ref() == Some(bypass))
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use infiltrator_contract::system_proxy::{
        SystemProxyObservation, SystemProxyOwnership, SystemProxyRecoveryStatus,
        SystemProxyStatus,
    };
    use std::sync::atomic::AtomicUsize;

    struct TestPort {
        observation: Mutex<SystemProxyObservation>,
        apply_calls: AtomicUsize,
        snapshot_calls: AtomicUsize,
        arm_calls: AtomicUsize,
        clear_calls: AtomicUsize,
        target: Arc<Mutex<Option<SystemProxyDesiredState>>>,
        recovery_report: Mutex<Option<SystemProxyRecoveryReport>>,
    }

    #[async_trait]
    impl SystemProxyPort for TestPort {
        fn shared_target(&self) -> Arc<Mutex<Option<SystemProxyDesiredState>>> {
            self.target.clone()
        }

        async fn arm_recovery(
            &self,
            _previous: SystemProxyObservation,
            _desired: SystemProxyDesiredState,
        ) -> Result<(), PortError> {
            self.arm_calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        async fn clear_recovery(&self) -> Result<(), PortError> {
            self.clear_calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
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

        async fn apply(
            &self,
            endpoint: Option<String>,
            bypass: Option<String>,
        ) -> Result<(), PortError> {
            self.apply_calls.fetch_add(1, Ordering::SeqCst);
            let mut observation = self.observation.lock().expect("proxy lock");
            observation.endpoint = endpoint.clone();
            observation.bypass = bypass;
            observation.enabled = endpoint.is_some();
            Ok(())
        }
    }

    #[tokio::test]
    async fn application_applies_and_reads_back_proxy_state() {
        let port = Arc::new(TestPort {
            observation: Mutex::new(SystemProxyObservation::default()),
            apply_calls: AtomicUsize::new(0),
            snapshot_calls: AtomicUsize::new(0),
            arm_calls: AtomicUsize::new(0),
            clear_calls: AtomicUsize::new(0),
            target: Arc::new(Mutex::new(None)),
            recovery_report: Mutex::new(None),
        });
        let application = SystemProxyApplication::new(port.clone());
        let snapshot = application
            .set_enabled(
                true,
                Some("127.0.0.1:7890".to_owned()),
                Some("localhost".to_owned()),
            )
            .await
            .expect("proxy apply");
        assert_eq!(snapshot.status, SystemProxyStatus::Enabled);
        assert_eq!(snapshot.ownership, SystemProxyOwnership::Owned);
        assert_eq!(port.apply_calls.load(Ordering::SeqCst), 1);
        assert_eq!(port.arm_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn application_caches_surface_observations() {
        let port = Arc::new(TestPort {
            observation: Mutex::new(SystemProxyObservation::default()),
            apply_calls: AtomicUsize::new(0),
            snapshot_calls: AtomicUsize::new(0),
            arm_calls: AtomicUsize::new(0),
            clear_calls: AtomicUsize::new(0),
            target: Arc::new(Mutex::new(None)),
            recovery_report: Mutex::new(None),
        });
        let application = SystemProxyApplication::new(port.clone());
        let first = application.snapshot_cached().await;
        let second = application.snapshot_cached().await;
        assert_eq!(first, second);
        assert_eq!(port.apply_calls.load(Ordering::SeqCst), 0);
        assert_eq!(port.snapshot_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn application_repairs_external_override_and_reports_one_warning_state() {
        let port = Arc::new(TestPort {
            observation: Mutex::new(SystemProxyObservation::default()),
            apply_calls: AtomicUsize::new(0),
            snapshot_calls: AtomicUsize::new(0),
            arm_calls: AtomicUsize::new(0),
            clear_calls: AtomicUsize::new(0),
            target: Arc::new(Mutex::new(None)),
            recovery_report: Mutex::new(None),
        });
        let application = SystemProxyApplication::new(port.clone());
        application
            .set_enabled(
                true,
                Some("127.0.0.1:7890".to_owned()),
                Some("localhost".to_owned()),
            )
            .await
            .expect("initial proxy apply");
        *port.observation.lock().expect("proxy lock") = SystemProxyObservation {
            enabled: true,
            endpoint: Some("127.0.0.1:9999".to_owned()),
            bypass: Some("example.com".to_owned()),
        };

        let snapshot = application.snapshot().await;
        assert_eq!(snapshot.status, SystemProxyStatus::Enabled);
        assert_eq!(snapshot.ownership, SystemProxyOwnership::Repaired);
        assert_eq!(snapshot.repair_count, 1);
        assert_eq!(snapshot.endpoint.as_deref(), Some("127.0.0.1:7890"));
        assert_eq!(port.apply_calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn separate_compositions_share_proxy_ownership_through_the_port() {
        let port = Arc::new(TestPort {
            observation: Mutex::new(SystemProxyObservation::default()),
            apply_calls: AtomicUsize::new(0),
            snapshot_calls: AtomicUsize::new(0),
            arm_calls: AtomicUsize::new(0),
            clear_calls: AtomicUsize::new(0),
            target: Arc::new(Mutex::new(None)),
            recovery_report: Mutex::new(None),
        });
        let command_application = SystemProxyApplication::new(port.clone());
        let surface_application = SystemProxyApplication::new(port.clone());
        command_application
            .set_enabled(true, Some("127.0.0.1:7890".to_owned()), None)
            .await
            .expect("command composition apply");

        let snapshot = surface_application.snapshot().await;
        assert_eq!(snapshot.ownership, SystemProxyOwnership::Owned);
        assert_eq!(port.apply_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn disabling_proxy_releases_ownership_and_clears_recovery() {
        let port = Arc::new(TestPort {
            observation: Mutex::new(SystemProxyObservation {
                enabled: true,
                endpoint: Some("127.0.0.1:7890".to_owned()),
                bypass: None,
            }),
            apply_calls: AtomicUsize::new(0),
            snapshot_calls: AtomicUsize::new(0),
            arm_calls: AtomicUsize::new(0),
            clear_calls: AtomicUsize::new(0),
            target: Arc::new(Mutex::new(Some(SystemProxyDesiredState {
                enabled: true,
                endpoint: Some("127.0.0.1:7890".to_owned()),
                bypass: None,
            }))),
            recovery_report: Mutex::new(None),
        });
        let application = SystemProxyApplication::new(port.clone());
        let snapshot = application
            .set_enabled(false, None, None)
            .await
            .expect("proxy disable");

        assert_eq!(snapshot.status, SystemProxyStatus::Disabled);
        assert_eq!(snapshot.ownership, SystemProxyOwnership::Unmanaged);
        assert_eq!(port.apply_calls.load(Ordering::SeqCst), 1);
        assert_eq!(port.arm_calls.load(Ordering::SeqCst), 0);
        assert_eq!(port.clear_calls.load(Ordering::SeqCst), 1);
        assert!(port.target.lock().expect("proxy target lock").is_none());
    }

    #[tokio::test]
    async fn application_publishes_typed_startup_recovery_result() {
        let port = Arc::new(TestPort {
            observation: Mutex::new(SystemProxyObservation::default()),
            apply_calls: AtomicUsize::new(0),
            snapshot_calls: AtomicUsize::new(0),
            arm_calls: AtomicUsize::new(0),
            clear_calls: AtomicUsize::new(0),
            target: Arc::new(Mutex::new(None)),
            recovery_report: Mutex::new(Some(SystemProxyRecoveryReport::SkippedLiveOwner {
                owner_pid: 4242,
            })),
        });
        let application = SystemProxyApplication::new(port);
        let snapshot = application.recover_orphaned().await;
        assert!(matches!(
            snapshot.status,
            SystemProxyRecoveryStatus::SkippedLiveOwner { owner_pid: 4242 }
        ));
        assert_eq!(application.recovery_snapshot(), snapshot);
    }
}
