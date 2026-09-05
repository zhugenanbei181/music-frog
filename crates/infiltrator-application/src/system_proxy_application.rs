//! System HTTP/SOCKS proxy use-case over a host-owned port.

use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::system_proxy::SystemProxySnapshot;
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
}

impl SystemProxyApplication {
    pub fn new(port: Arc<dyn SystemProxyPort>) -> Self {
        Self {
            port,
            next_revision: Arc::new(AtomicU64::new(1)),
            last_snapshot: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn snapshot(&self) -> SystemProxySnapshot {
        let snapshot = self.snapshot_fresh().await;
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
        self.port
            .apply(endpoint, bypass)
            .await
            .map_err(Failure::from)?;
        let observation = self.port.snapshot().await.map_err(Failure::from)?;
        if observation.enabled != enabled || (enabled && observation.endpoint.is_none()) {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                format!(
                    "system proxy readback mismatch: requested enabled={enabled}, observed enabled={} endpoint={:?}",
                    observation.enabled, observation.endpoint
                ),
                true,
            ));
        }
        let snapshot = SystemProxySnapshot::from_observation(
            self.next_revision.fetch_add(1, Ordering::Relaxed),
            observation,
        );
        self.cache(snapshot.clone());
        Ok(snapshot)
    }

    async fn snapshot_fresh(&self) -> SystemProxySnapshot {
        let revision = self.next_revision.fetch_add(1, Ordering::Relaxed);
        match self.port.snapshot().await {
            Ok(observation) => SystemProxySnapshot::from_observation(revision, observation),
            Err(PortError::Unsupported { reason, .. }) => {
                SystemProxySnapshot::unsupported(revision, reason)
            }
            Err(error) => SystemProxySnapshot::failed(revision, Failure::from(error)),
        }
    }

    fn cache(&self, snapshot: SystemProxySnapshot) {
        *self
            .last_snapshot
            .lock()
            .expect("system proxy snapshot cache lock") = Some((Instant::now(), snapshot));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use infiltrator_contract::system_proxy::{SystemProxyObservation, SystemProxyStatus};
    use std::sync::atomic::AtomicUsize;

    struct TestPort {
        observation: Mutex<SystemProxyObservation>,
        apply_calls: AtomicUsize,
        snapshot_calls: AtomicUsize,
    }

    #[async_trait]
    impl SystemProxyPort for TestPort {
        async fn snapshot(&self) -> Result<SystemProxyObservation, PortError> {
            self.snapshot_calls.fetch_add(1, Ordering::SeqCst);
            Ok(self.observation.lock().expect("proxy lock").clone())
        }

        async fn apply(
            &self,
            endpoint: Option<String>,
            _bypass: Option<String>,
        ) -> Result<(), PortError> {
            self.apply_calls.fetch_add(1, Ordering::SeqCst);
            let mut observation = self.observation.lock().expect("proxy lock");
            observation.endpoint = endpoint.clone();
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
        assert_eq!(port.apply_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn application_caches_surface_observations() {
        let port = Arc::new(TestPort {
            observation: Mutex::new(SystemProxyObservation::default()),
            apply_calls: AtomicUsize::new(0),
            snapshot_calls: AtomicUsize::new(0),
        });
        let application = SystemProxyApplication::new(port.clone());
        let first = application.snapshot_cached().await;
        let second = application.snapshot_cached().await;
        assert_eq!(first, second);
        assert_eq!(port.apply_calls.load(Ordering::SeqCst), 0);
        assert_eq!(port.snapshot_calls.load(Ordering::SeqCst), 1);
    }
}
