//! DNS cache maintenance use-case.
//!
//! One flush request covers both targets:
//! * the running core's Fake-IP mapping table through the controller gateway;
//! * the operating system resolver cache through the host
//!   [`SystemDnsCachePort`](infiltrator_ports::system_dns_cache::SystemDnsCachePort).
//!
//! The report keeps per-target outcomes, so a host without a drivable OS
//! cache flush reports a typed unsupported instead of a fabricated success.
//! The command handler and the surface reader share one instance, which is how
//! the honest last-flush report reaches both surfaces.

use infiltrator_contract::dns::{DnsCacheFlushReport, DnsFlushOutcome};
use infiltrator_contract::error::Failure;
use infiltrator_ports::error::PortError;
use infiltrator_ports::runtime_gateway::RuntimeGateway;
use infiltrator_ports::system_dns_cache::SystemDnsCachePort;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct DnsCacheApplication {
    runtime: Option<Arc<dyn RuntimeGateway>>,
    system_cache: Option<Arc<dyn SystemDnsCachePort>>,
    last: Arc<Mutex<DnsCacheFlushReport>>,
}

impl DnsCacheApplication {
    pub fn new(
        runtime: Option<Arc<dyn RuntimeGateway>>,
        system_cache: Option<Arc<dyn SystemDnsCachePort>>,
    ) -> Self {
        Self {
            runtime,
            system_cache,
            last: Arc::new(Mutex::new(DnsCacheFlushReport::default())),
        }
    }

    /// A host that can drive neither target still keeps an honest report.
    pub fn unconfigured() -> Self {
        Self::new(None, None)
    }

    /// The honest report of the last flush (or `NotRequested`).
    pub fn last_report(&self) -> DnsCacheFlushReport {
        self.last.lock().expect("dns cache report lock").clone()
    }

    /// Flush the Fake-IP table and, when the host provides one, the OS cache.
    ///
    /// The returned report is also stored as the last report. A partially
    /// unsupported host is not an error: the report says which target ran.
    pub async fn flush_all(&self) -> Result<DnsCacheFlushReport, Failure> {
        let fake_ip = match self.runtime.as_ref() {
            Some(runtime) => outcome_from_result(runtime.flush_fakeip_cache().await),
            None => DnsFlushOutcome::Unsupported {
                reason: "no running core controller is attached".to_owned(),
            },
        };
        let os_cache = match self.system_cache.as_ref() {
            Some(port) => match port.flush_system_cache().await {
                Ok(true) => DnsFlushOutcome::Flushed,
                Ok(false) => DnsFlushOutcome::Unsupported {
                    reason: "host reported no drivable OS DNS cache flush".to_owned(),
                },
                Err(error) => outcome_from_result::<()>(Err(error)),
            },
            None => DnsFlushOutcome::Unsupported {
                reason: "host did not provide a system DNS cache adapter".to_owned(),
            },
        };
        let report = DnsCacheFlushReport { fake_ip, os_cache };
        *self.last.lock().expect("dns cache report lock") = report.clone();
        Ok(report)
    }
}

fn outcome_from_result<T>(result: Result<T, PortError>) -> DnsFlushOutcome {
    match result {
        Ok(_) => DnsFlushOutcome::Flushed,
        Err(PortError::Unsupported { reason, .. }) => DnsFlushOutcome::Unsupported { reason },
        Err(error) => DnsFlushOutcome::Failed {
            message: error.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use infiltrator_contract::capability::Capability;
    use infiltrator_domain::runtime::ConfigSnapshot;
    use infiltrator_ports::runtime_gateway::RuntimeStream;
    use std::collections::HashMap;

    struct FakeGateway {
        flushed: bool,
    }

    #[async_trait]
    impl RuntimeGateway for FakeGateway {
        async fn get_config(&self) -> Result<ConfigSnapshot, PortError> {
            Ok(ConfigSnapshot::default())
        }
        async fn patch_config(&self, _updates: serde_json::Value) -> Result<(), PortError> {
            Ok(())
        }
        async fn set_proxy_mode(
            &self,
            _mode: infiltrator_contract::command::ProxyMode,
        ) -> Result<(), PortError> {
            Ok(())
        }
        async fn get_proxies(
            &self,
        ) -> Result<HashMap<String, infiltrator_domain::proxy::Proxy>, PortError> {
            Ok(HashMap::new())
        }
        async fn switch_proxy(&self, _group: &str, _proxy: &str) -> Result<(), PortError> {
            Ok(())
        }
        async fn test_delay(
            &self,
            _proxy: &str,
            _url: &str,
            _timeout_ms: u32,
        ) -> Result<u32, PortError> {
            Ok(0)
        }
        async fn get_proxy_providers(
            &self,
        ) -> Result<Vec<infiltrator_domain::runtime::ProxyProvider>, PortError> {
            Ok(Vec::new())
        }
        async fn get_rule_providers(
            &self,
        ) -> Result<Vec<infiltrator_domain::runtime::RuleProvider>, PortError> {
            Ok(Vec::new())
        }
        async fn update_proxy_provider(&self, _name: &str) -> Result<(), PortError> {
            Ok(())
        }
        async fn update_rule_provider(&self, _name: &str) -> Result<(), PortError> {
            Ok(())
        }
        async fn flush_fakeip_cache(&self) -> Result<(), PortError> {
            self.flushed.then_some(()).ok_or_else(|| {
                PortError::unsupported(Capability::Dns, "controller rejected the flush")
            })
        }
        async fn get_connections(
            &self,
        ) -> Result<infiltrator_domain::runtime::ConnectionSnapshot, PortError> {
            Ok(infiltrator_domain::runtime::ConnectionSnapshot::default())
        }
        async fn get_memory(&self) -> Result<infiltrator_domain::runtime::MemoryData, PortError> {
            Ok(infiltrator_domain::runtime::MemoryData::default())
        }
        async fn close_connection(&self, _id: &str) -> Result<(), PortError> {
            Ok(())
        }
        async fn close_all_connections(&self) -> Result<(), PortError> {
            Ok(())
        }
        async fn stream_logs(
            &self,
            _level: Option<String>,
        ) -> Result<RuntimeStream<String>, PortError> {
            Ok(Box::pin(futures_util::stream::empty()))
        }
        async fn stream_traffic(
            &self,
        ) -> Result<RuntimeStream<infiltrator_domain::runtime::TrafficData>, PortError> {
            Ok(Box::pin(futures_util::stream::empty()))
        }
        async fn stream_connections(
            &self,
        ) -> Result<RuntimeStream<infiltrator_domain::runtime::ConnectionSnapshot>, PortError>
        {
            Ok(Box::pin(futures_util::stream::empty()))
        }
    }

    struct FakeSystemCache {
        supported: bool,
    }

    #[async_trait]
    impl SystemDnsCachePort for FakeSystemCache {
        async fn flush_system_cache(&self) -> Result<bool, PortError> {
            Ok(self.supported)
        }
    }

    #[tokio::test]
    async fn reports_both_targets_honestly() {
        let application = DnsCacheApplication::new(
            Some(Arc::new(FakeGateway { flushed: true })),
            Some(Arc::new(FakeSystemCache { supported: true })),
        );
        let report = application.flush_all().await.expect("flush");
        assert_eq!(report.fake_ip, DnsFlushOutcome::Flushed);
        assert_eq!(report.os_cache, DnsFlushOutcome::Flushed);
        assert_eq!(application.last_report(), report);
    }

    #[tokio::test]
    async fn a_host_without_the_os_adapter_reports_typed_unsupported() {
        let application = DnsCacheApplication::new(
            Some(Arc::new(FakeGateway { flushed: true })),
            Some(Arc::new(FakeSystemCache { supported: false })),
        );
        let report = application.flush_all().await.expect("flush");
        assert!(report.fake_ip.is_flushed());
        assert!(matches!(
            report.os_cache,
            DnsFlushOutcome::Unsupported { .. }
        ));
    }

    #[tokio::test]
    async fn a_failing_controller_is_reported_not_hidden() {
        let application =
            DnsCacheApplication::new(Some(Arc::new(FakeGateway { flushed: false })), None);
        let report = application.flush_all().await.expect("flush");
        assert!(matches!(
            report.fake_ip,
            DnsFlushOutcome::Unsupported { .. }
        ));
        assert!(matches!(
            report.os_cache,
            DnsFlushOutcome::Unsupported { .. }
        ));
        assert_eq!(application.last_report(), report);
    }
}
