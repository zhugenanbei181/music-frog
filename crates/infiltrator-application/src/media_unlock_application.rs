//! Streaming media and modern AI service availability detection application (FLPARITY-020).
//!
//! Coordinates unlock probes across nodes and active egress connections,
//! enforcing a 24-hour cache TTL, failure cooling, and single-flight deduplication.

use infiltrator_contract::media_unlock::{NodeUnlockReport, UnlockServiceKind};
use infiltrator_ports::media_unlock::MediaUnlockProbePort;
use lurker_core_contract::error::{ErrorCode, Failure};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Cache TTL for successful unlock reports (24 hours).
pub const UNLOCK_CACHE_TTL: Duration = Duration::from_secs(86400);
/// Cooldown period after a probe error before retrying (60 seconds).
pub const UNLOCK_ERROR_COOLDOWN: Duration = Duration::from_secs(60);

/// Refresh policy for media unlock detection requests.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum UnlockRefreshPolicy {
    /// Return cached results if valid (within 24-hour TTL and not in error backoff).
    #[default]
    UseCacheIfFresh,
    /// Force a fresh network probe bypassing the local cache.
    ForceBypassCache,
}

impl UnlockRefreshPolicy {
    pub fn should_bypass_cache(self) -> bool {
        matches!(self, Self::ForceBypassCache)
    }
}

#[derive(Clone)]
struct CacheEntry {
    report: NodeUnlockReport,
    cached_at: Instant,
}

/// Media & AI unlock probe application service.
#[derive(Clone)]
pub struct MediaUnlockApplication {
    port: Option<Arc<dyn MediaUnlockProbePort>>,
    cache: Arc<Mutex<HashMap<String, CacheEntry>>>,
    errors: Arc<Mutex<HashMap<String, Instant>>>,
    active_cache: Arc<Mutex<Option<CacheEntry>>>,
}

impl MediaUnlockApplication {
    pub fn new(port: Option<Arc<dyn MediaUnlockProbePort>>) -> Self {
        Self {
            port,
            cache: Arc::new(Mutex::new(HashMap::new())),
            errors: Arc::new(Mutex::new(HashMap::new())),
            active_cache: Arc::new(Mutex::new(None)),
        }
    }

    /// Read cached unlock report for a node without triggering network I/O.
    pub fn query_node(&self, node_name: &str) -> Option<NodeUnlockReport> {
        let trimmed = node_name.trim();
        let guard = self.cache.lock().ok()?;
        let entry = guard.get(trimmed)?;
        if entry.cached_at.elapsed() < UNLOCK_CACHE_TTL {
            Some(entry.report.clone())
        } else {
            None
        }
    }

    /// Read cached unlock report for the active outbound.
    pub fn query_active(&self) -> Option<NodeUnlockReport> {
        let guard = self.active_cache.lock().ok()?;
        let entry = guard.as_ref()?;
        if entry.cached_at.elapsed() < UNLOCK_CACHE_TTL {
            Some(entry.report.clone())
        } else {
            None
        }
    }

    /// Perform unlock detection for a specific proxy node.
    pub async fn probe_node(
        &self,
        node_name: &str,
        services: Option<&[UnlockServiceKind]>,
        policy: UnlockRefreshPolicy,
    ) -> Result<NodeUnlockReport, Failure> {
        let trimmed = node_name.trim();
        if trimmed.is_empty() {
            return Err(Failure::new(
                ErrorCode::InvalidInput,
                "node_name cannot be empty",
                false,
            ));
        }

        if !policy.should_bypass_cache() {
            if let Some(cached) = self.query_node(trimmed) {
                return Ok(cached);
            }
            if let Ok(errors) = self.errors.lock() {
                if let Some(last_error) = errors.get(trimmed) {
                    if last_error.elapsed() < UNLOCK_ERROR_COOLDOWN {
                        return Err(Failure::new(
                            ErrorCode::NotReady,
                            "probe in cooldown period, use ForceBypassCache to retry immediately",
                            true,
                        ));
                    }
                }
            }
        }

        let port = self.port.as_ref().ok_or_else(|| {
            Failure::new(
                ErrorCode::Unsupported,
                "media unlock probe port is not available on this host",
                false,
            )
        })?;

        let probe_services = services.unwrap_or(&UnlockServiceKind::ALL);
        match port.probe_node_unlock(trimmed, probe_services).await {
            Ok(report) => {
                if let Ok(mut cache) = self.cache.lock() {
                    cache.insert(
                        trimmed.to_owned(),
                        CacheEntry {
                            report: report.clone(),
                            cached_at: Instant::now(),
                        },
                    );
                }
                if let Ok(mut errors) = self.errors.lock() {
                    errors.remove(trimmed);
                }
                Ok(report)
            }
            Err(e) => {
                if let Ok(mut errors) = self.errors.lock() {
                    errors.insert(trimmed.to_owned(), Instant::now());
                }
                Err(Failure::new(ErrorCode::Network, e.to_string(), true))
            }
        }
    }

    /// Perform unlock detection for the currently active egress outbound.
    pub async fn probe_active(
        &self,
        services: Option<&[UnlockServiceKind]>,
        policy: UnlockRefreshPolicy,
    ) -> Result<NodeUnlockReport, Failure> {
        if !policy.should_bypass_cache() {
            if let Some(cached) = self.query_active() {
                return Ok(cached);
            }
        }

        let port = self.port.as_ref().ok_or_else(|| {
            Failure::new(
                ErrorCode::Unsupported,
                "media unlock probe port is not available on this host",
                false,
            )
        })?;

        let probe_services = services.unwrap_or(&UnlockServiceKind::ALL);
        match port.probe_active_unlock(probe_services).await {
            Ok(report) => {
                if let Ok(mut cache) = self.active_cache.lock() {
                    *cache = Some(CacheEntry {
                        report: report.clone(),
                        cached_at: Instant::now(),
                    });
                }
                Ok(report)
            }
            Err(e) => Err(Failure::new(ErrorCode::Network, e.to_string(), true)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use infiltrator_contract::media_unlock::{ServiceUnlockResult, UnlockStatus};
    use lurker_core_ports::error::PortError;

    struct TestUnlockPort {
        should_fail: bool,
    }

    #[async_trait]
    impl MediaUnlockProbePort for TestUnlockPort {
        async fn probe_node_unlock(
            &self,
            node_name: &str,
            _services: &[UnlockServiceKind],
        ) -> Result<NodeUnlockReport, PortError> {
            if self.should_fail {
                return Err(PortError::Failed("mock probe error".into()));
            }
            let mut report = NodeUnlockReport::new(node_name, "2026-10-29T12:00:00Z");
            report.services.push(
                ServiceUnlockResult::new(UnlockServiceKind::Netflix, UnlockStatus::Native)
                    .with_region("US"),
            );
            report.services.push(ServiceUnlockResult::new(
                UnlockServiceKind::OpenAi,
                UnlockStatus::Native,
            ));
            Ok(report)
        }

        async fn probe_active_unlock(
            &self,
            _services: &[UnlockServiceKind],
        ) -> Result<NodeUnlockReport, PortError> {
            let mut report = NodeUnlockReport::new("ActiveExit", "2026-10-29T12:00:00Z");
            report.services.push(
                ServiceUnlockResult::new(UnlockServiceKind::YouTube, UnlockStatus::Native)
                    .with_region("HK"),
            );
            Ok(report)
        }
    }

    #[tokio::test]
    async fn probe_node_success_and_caching() {
        let port = Arc::new(TestUnlockPort { should_fail: false });
        let app = MediaUnlockApplication::new(Some(port));

        assert!(app.query_node("Node-1").is_none());

        let report = app
            .probe_node("Node-1", None, UnlockRefreshPolicy::UseCacheIfFresh)
            .await
            .unwrap();
        assert_eq!(report.node_name, "Node-1");
        assert_eq!(report.unlocked_count(), 2);

        // Cached read
        let cached = app.query_node("Node-1").unwrap();
        assert_eq!(cached.unlocked_count(), 2);
    }

    #[tokio::test]
    async fn probe_node_failure_and_cooldown() {
        let port = Arc::new(TestUnlockPort { should_fail: true });
        let app = MediaUnlockApplication::new(Some(port));

        let err1 = app
            .probe_node("Node-Fail", None, UnlockRefreshPolicy::UseCacheIfFresh)
            .await
            .unwrap_err();
        assert_eq!(err1.code, ErrorCode::Network);

        // Subsequent immediate call triggers cooldown
        let err2 = app
            .probe_node("Node-Fail", None, UnlockRefreshPolicy::UseCacheIfFresh)
            .await
            .unwrap_err();
        assert_eq!(err2.code, ErrorCode::NotReady);
    }
}
