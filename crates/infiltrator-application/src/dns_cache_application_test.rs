use super::*;
use async_trait::async_trait;
#[cfg(test)]
use futures_util::stream::empty;
use infiltrator_contract::capability::Capability;
#[cfg(test)]
use infiltrator_contract::command::ProxyMode;
#[cfg(test)]
use infiltrator_domain::proxy::Proxy;
use infiltrator_domain::runtime::ConfigSnapshot;
#[cfg(test)]
use infiltrator_domain::runtime::ConnectionSnapshot;
#[cfg(test)]
use infiltrator_domain::runtime::MemoryData;
#[cfg(test)]
use infiltrator_domain::runtime::ProxyProvider;
#[cfg(test)]
use infiltrator_domain::runtime::RuleProvider;
#[cfg(test)]
use infiltrator_domain::runtime::TrafficData;
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
    async fn set_proxy_mode(&self, _mode: ProxyMode) -> Result<(), PortError> {
        Ok(())
    }
    async fn get_proxies(&self) -> Result<HashMap<String, Proxy>, PortError> {
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
    async fn get_proxy_providers(&self) -> Result<Vec<ProxyProvider>, PortError> {
        Ok(Vec::new())
    }
    async fn get_rule_providers(&self) -> Result<Vec<RuleProvider>, PortError> {
        Ok(Vec::new())
    }
    async fn update_proxy_provider(&self, _name: &str) -> Result<(), PortError> {
        Ok(())
    }
    async fn update_rule_provider(&self, _name: &str) -> Result<(), PortError> {
        Ok(())
    }
    async fn flush_fakeip_cache(&self) -> Result<(), PortError> {
        self.flushed
            .then_some(())
            .ok_or_else(|| PortError::unsupported(Capability::Dns, "controller rejected the flush"))
    }
    async fn get_connections(&self) -> Result<ConnectionSnapshot, PortError> {
        Ok(ConnectionSnapshot::default())
    }
    async fn get_memory(&self) -> Result<MemoryData, PortError> {
        Ok(MemoryData::default())
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
        Ok(Box::pin(empty()))
    }
    async fn stream_traffic(&self) -> Result<RuntimeStream<TrafficData>, PortError> {
        Ok(Box::pin(empty()))
    }
    async fn stream_connections(&self) -> Result<RuntimeStream<ConnectionSnapshot>, PortError> {
        Ok(Box::pin(empty()))
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
