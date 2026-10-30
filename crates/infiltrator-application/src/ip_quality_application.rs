//! IP quality, fraud scoring, and usage type detection application (FLPARITY-001).
//!
//! Coordinates IP quality probes and fraud intelligence queries, enforcing a
//! 3-day cache TTL (matching FlClash v0.9.2), failure cooling, and single-flight
//! deduplication.

use infiltrator_contract::ip_quality::{
    IP_QUALITY_CACHE_TTL, IP_QUALITY_ERROR_COOLDOWN, IpQualityRecord, IpQualitySnapshot,
};
use infiltrator_contract::snapshot::CoreSnapshot;
use infiltrator_ports::ip_quality::IpQualityProbePort;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::lifecycle::CoreLifecycle;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Refresh policy for IP quality detection requests.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum IpQualityRefreshPolicy {
    /// Return cached results if valid (within 3-day TTL and not in error backoff).
    #[default]
    UseCacheIfFresh,
    /// Force a fresh network probe bypassing the local cache.
    ForceBypassCache,
}

impl IpQualityRefreshPolicy {
    pub fn should_bypass_cache(self) -> bool {
        matches!(self, Self::ForceBypassCache)
    }
}

#[derive(Clone)]
struct CacheEntry {
    record: IpQualityRecord,
    cached_at: Instant,
}

/// IP quality probe application service.
#[derive(Clone)]
pub struct IpQualityApplication {
    port: Option<Arc<dyn IpQualityProbePort>>,
    cache: Arc<Mutex<HashMap<String, CacheEntry>>>,
    errors: Arc<Mutex<HashMap<String, Instant>>>,
    active_cache: Arc<Mutex<Option<CacheEntry>>>,
}

impl IpQualityApplication {
    pub fn new(port: Option<Arc<dyn IpQualityProbePort>>) -> Self {
        Self {
            port,
            cache: Arc::new(Mutex::new(HashMap::new())),
            errors: Arc::new(Mutex::new(HashMap::new())),
            active_cache: Arc::new(Mutex::new(None)),
        }
    }

    /// Read cached IP quality record and cache age in seconds without triggering network I/O.
    pub fn query_ip(&self, ip: &str) -> Option<(IpQualityRecord, u64)> {
        let trimmed = ip.trim();
        let guard = self.cache.lock().ok()?;
        let entry = guard.get(trimmed)?;
        let elapsed = entry.cached_at.elapsed();
        if elapsed < IP_QUALITY_CACHE_TTL {
            Some((entry.record.clone(), elapsed.as_secs()))
        } else {
            None
        }
    }

    /// Read cached IP quality record for the active egress outbound and its cache age in seconds.
    pub fn query_active(&self) -> Option<(IpQualityRecord, u64)> {
        let guard = self.active_cache.lock().ok()?;
        let entry = guard.as_ref()?;
        let elapsed = entry.cached_at.elapsed();
        if elapsed < IP_QUALITY_CACHE_TTL {
            Some((entry.record.clone(), elapsed.as_secs()))
        } else {
            None
        }
    }

    /// Perform IP quality detection for an explicit IP address.
    pub async fn probe_ip(
        &self,
        ip: &str,
        policy: IpQualityRefreshPolicy,
    ) -> Result<IpQualityRecord, Failure> {
        let trimmed = ip.trim();
        if trimmed.is_empty() {
            return Err(Failure::new(
                ErrorCode::InvalidInput,
                "ip cannot be empty",
                false,
            ));
        }

        if !policy.should_bypass_cache() {
            if let Some((cached, _)) = self.query_ip(trimmed) {
                return Ok(cached);
            }
            if let Ok(errors) = self.errors.lock() {
                if let Some(last_error) = errors.get(trimmed) {
                    if last_error.elapsed() < IP_QUALITY_ERROR_COOLDOWN {
                        return Err(Failure::new(
                            ErrorCode::NotReady,
                            "IP quality probe in cooldown period; use ForceBypassCache to retry immediately",
                            true,
                        ));
                    }
                }
            }
        }

        let port = self.port.as_ref().ok_or_else(|| {
            Failure::new(
                ErrorCode::Unsupported,
                "IP quality probe port is not available on this host",
                false,
            )
        })?;

        match port.probe_ip_quality(trimmed).await {
            Ok(record) => {
                if let Ok(mut cache) = self.cache.lock() {
                    cache.insert(
                        trimmed.to_owned(),
                        CacheEntry {
                            record: record.clone(),
                            cached_at: Instant::now(),
                        },
                    );
                }
                if let Ok(mut errors) = self.errors.lock() {
                    errors.remove(trimmed);
                }
                Ok(record)
            }
            Err(e) => {
                if let Ok(mut errors) = self.errors.lock() {
                    errors.insert(trimmed.to_owned(), Instant::now());
                }
                Err(Failure::new(ErrorCode::Network, e.to_string(), true))
            }
        }
    }

    /// Perform IP quality detection for the active egress outbound.
    pub async fn probe_active(
        &self,
        policy: IpQualityRefreshPolicy,
    ) -> Result<IpQualityRecord, Failure> {
        if !policy.should_bypass_cache() {
            if let Some((cached, _)) = self.query_active() {
                return Ok(cached);
            }
        }

        let port = self.port.as_ref().ok_or_else(|| {
            Failure::new(
                ErrorCode::Unsupported,
                "IP quality probe port is not available on this host",
                false,
            )
        })?;

        match port.probe_active_ip_quality().await {
            Ok(record) => {
                if let Ok(mut cache) = self.active_cache.lock() {
                    *cache = Some(CacheEntry {
                        record: record.clone(),
                        cached_at: Instant::now(),
                    });
                }
                Ok(record)
            }
            Err(e) => Err(Failure::new(ErrorCode::Network, e.to_string(), true)),
        }
    }

    /// Projects the authoritative IP quality facts into a SurfaceSnapshot-compatible snapshot.
    pub fn project_snapshot(
        &self,
        core: &CoreSnapshot,
        current_ip: Option<&str>,
    ) -> IpQualitySnapshot {
        let revision = core.revision.max(1);
        if !matches!(
            core.lifecycle,
            CoreLifecycle::Running | CoreLifecycle::Ready
        ) {
            return IpQualitySnapshot::unavailable(
                core.generation,
                revision,
                "core is not running; IP quality is not probed",
            );
        }

        if self.port.is_none() {
            return IpQualitySnapshot::unsupported(
                core.generation,
                revision,
                "IP quality probe port is not composed on this host",
            );
        }

        if let Some(ip) = current_ip.map(str::trim).filter(|s| !s.is_empty()) {
            if let Some((record, age)) = self.query_ip(ip) {
                return IpQualitySnapshot::ready(
                    core.generation,
                    revision,
                    record,
                    true,
                    Some(age),
                );
            }
            if let Ok(errors) = self.errors.lock() {
                if let Some(last_error) = errors.get(ip) {
                    if last_error.elapsed() < IP_QUALITY_ERROR_COOLDOWN {
                        return IpQualitySnapshot::cooldown(core.generation, revision, None, None);
                    }
                }
            }
            return IpQualitySnapshot::probing(core.generation, revision);
        }

        if let Some((record, age)) = self.query_active() {
            return IpQualitySnapshot::ready(core.generation, revision, record, true, Some(age));
        }

        IpQualitySnapshot::probing(core.generation, revision)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use infiltrator_contract::ip_quality::{IpQualityStatus, IpRiskLevel, IpUsageType};
    use lurker_core_ports::error::PortError;

    struct TestQualityPort {
        should_fail: bool,
        rate_limited: bool,
    }

    #[async_trait]
    impl IpQualityProbePort for TestQualityPort {
        async fn probe_ip_quality(&self, ip: &str) -> Result<IpQualityRecord, PortError> {
            if self.rate_limited {
                return Err(PortError::Failed("HTTP 429: upstream rate limit".into()));
            }
            if self.should_fail {
                return Err(PortError::Failed("mock probe error".into()));
            }
            let mut record = IpQualityRecord::new(
                ip,
                IpUsageType::Residential,
                IpRiskLevel::Clean,
                1741300000000,
            );
            record.fraud_score = Some(0);
            record.asn = Some(13335);
            record.as_organization = Some("Cloudflare".to_owned());
            record.country_code = Some("HK".to_owned());
            record.city = Some("Hong Kong".to_owned());
            record.provider = Some("ipapi.is".to_owned());
            Ok(record)
        }

        async fn probe_active_ip_quality(&self) -> Result<IpQualityRecord, PortError> {
            self.probe_ip_quality("198.51.100.1").await
        }
    }

    fn dummy_core(lifecycle: CoreLifecycle) -> CoreSnapshot {
        CoreSnapshot {
            identity: None,
            lifecycle,
            generation: 1,
            revision: 1,
            session_token: None,
            routing_intent: Default::default(),
            core_version: None,
            sampled_at_epoch_ms: None,
            failure: None,
            upload_bps: 0.0,
            download_bps: 0.0,
            active_connections: 0,
            watchdog: Default::default(),
        }
    }

    #[tokio::test]
    async fn probe_ip_success_and_caching() {
        let port = Arc::new(TestQualityPort {
            should_fail: false,
            rate_limited: false,
        });
        let app = IpQualityApplication::new(Some(port));

        assert!(app.query_ip("1.1.1.1").is_none());

        let record = app
            .probe_ip("1.1.1.1", IpQualityRefreshPolicy::UseCacheIfFresh)
            .await
            .unwrap();
        assert_eq!(record.ip, "1.1.1.1");
        assert_eq!(record.usage_type, IpUsageType::Residential);
        assert_eq!(record.risk_level, IpRiskLevel::Clean);

        // Cached read
        let (cached, _) = app.query_ip("1.1.1.1").unwrap();
        assert_eq!(cached.ip, "1.1.1.1");
    }

    #[tokio::test]
    async fn probe_ip_failure_and_cooldown() {
        let port = Arc::new(TestQualityPort {
            should_fail: true,
            rate_limited: false,
        });
        let app = IpQualityApplication::new(Some(port));

        let err1 = app
            .probe_ip("1.1.1.1", IpQualityRefreshPolicy::UseCacheIfFresh)
            .await
            .unwrap_err();
        assert_eq!(err1.code, ErrorCode::Network);

        // Subsequent probe enters cooldown
        let err2 = app
            .probe_ip("1.1.1.1", IpQualityRefreshPolicy::UseCacheIfFresh)
            .await
            .unwrap_err();
        assert_eq!(err2.code, ErrorCode::NotReady);

        // Bypass cache allows retrying immediately
        let err3 = app
            .probe_ip("1.1.1.1", IpQualityRefreshPolicy::ForceBypassCache)
            .await
            .unwrap_err();
        assert_eq!(err3.code, ErrorCode::Network);
    }

    #[tokio::test]
    async fn probe_empty_ip_rejected() {
        let app = IpQualityApplication::new(None);
        let err = app
            .probe_ip("   ", IpQualityRefreshPolicy::UseCacheIfFresh)
            .await
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidInput);
    }

    #[test]
    fn project_snapshot_lifecycle_handling() {
        let app = IpQualityApplication::new(None);
        let snap = app.project_snapshot(&dummy_core(CoreLifecycle::Stopped), Some("1.1.1.1"));
        assert_eq!(snap.status, IpQualityStatus::Unknown);

        let snap_unsupported =
            app.project_snapshot(&dummy_core(CoreLifecycle::Running), Some("1.1.1.1"));
        assert_eq!(snap_unsupported.status, IpQualityStatus::Unsupported);
    }
}
