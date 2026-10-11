use super::*;
use async_trait::async_trait;
use infiltrator_contract::capability::Capability;
use infiltrator_contract::dns_cache::DnsCacheOperationId;
use infiltrator_contract::error::ErrorCode;
use infiltrator_ports::controller_fake_ip_cache::{
    ControllerFakeIpCacheFlushReceipt, ControllerFakeIpCacheFlushRequest,
    ControllerFakeIpCacheFlushState, ControllerFakeIpCachePort,
};
use infiltrator_ports::system_dns_cache::{SystemDnsCacheFlush, SystemDnsCachePort};

#[derive(Clone, Copy)]
enum FakeFlushBehavior {
    Flushed,
    Unsupported,
    Failed,
}

struct FakeControllerCache {
    behavior: FakeFlushBehavior,
}

#[async_trait]
impl ControllerFakeIpCachePort for FakeControllerCache {
    async fn flush(
        &self,
        request: ControllerFakeIpCacheFlushRequest,
    ) -> Result<ControllerFakeIpCacheFlushReceipt, PortError> {
        match self.behavior {
            FakeFlushBehavior::Flushed => Ok(ControllerFakeIpCacheFlushReceipt {
                scope: request.scope,
                operation: request.operation,
                state: ControllerFakeIpCacheFlushState::Flushed,
            }),
            FakeFlushBehavior::Unsupported => Err(PortError::unsupported(
                Capability::Dns,
                "controller does not expose Fake-IP cache flush",
            )),
            FakeFlushBehavior::Failed => Err(PortError::Network(
                "injected controller flush failure".to_owned(),
            )),
        }
    }
}

struct FakeSystemCache {
    behavior: FakeFlushBehavior,
}

#[async_trait]
impl SystemDnsCachePort for FakeSystemCache {
    async fn flush_system_cache(&self) -> Result<SystemDnsCacheFlush, PortError> {
        match self.behavior {
            FakeFlushBehavior::Flushed => Ok(SystemDnsCacheFlush::Flushed),
            FakeFlushBehavior::Unsupported => Ok(SystemDnsCacheFlush::Unsupported),
            FakeFlushBehavior::Failed => Err(PortError::Network(
                "injected system DNS cache flush failure".to_owned(),
            )),
        }
    }
}

#[tokio::test]
async fn reports_both_targets_honestly() {
    let application = DnsCacheApplication::new(
        Some(Arc::new(FakeControllerCache {
            behavior: FakeFlushBehavior::Flushed,
        })),
        Some(Arc::new(FakeSystemCache {
            behavior: FakeFlushBehavior::Flushed,
        })),
    );
    let report = application
        .flush_with_id(
            crate::dns_cache_fixtures::test_core_scope(),
            DnsCacheOperationId(1),
        )
        .await
        .expect("flush");
    assert_eq!(report.fake_ip, DnsFlushOutcome::Flushed);
    assert_eq!(report.os_cache, DnsFlushOutcome::Flushed);
    assert_eq!(application.last_report(), report);
}

#[tokio::test]
async fn a_host_without_the_os_adapter_reports_typed_unsupported() {
    let application = DnsCacheApplication::new(
        Some(Arc::new(FakeControllerCache {
            behavior: FakeFlushBehavior::Flushed,
        })),
        Some(Arc::new(FakeSystemCache {
            behavior: FakeFlushBehavior::Unsupported,
        })),
    );
    let report = application
        .flush_with_id(
            crate::dns_cache_fixtures::test_core_scope(),
            DnsCacheOperationId(1),
        )
        .await
        .expect("flush");
    assert!(report.fake_ip.is_flushed());
    assert!(matches!(
        report.os_cache,
        DnsFlushOutcome::Unsupported { .. }
    ));
}

#[tokio::test]
async fn a_failing_controller_is_reported_not_hidden() {
    let application = DnsCacheApplication::new(
        Some(Arc::new(FakeControllerCache {
            behavior: FakeFlushBehavior::Unsupported,
        })),
        None,
    );
    let report = application
        .flush_with_id(
            crate::dns_cache_fixtures::test_core_scope(),
            DnsCacheOperationId(1),
        )
        .await
        .expect("flush");
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

#[tokio::test]
async fn a_system_adapter_failure_is_distinct_from_unsupported_and_is_readable() {
    let application = DnsCacheApplication::new(
        Some(Arc::new(FakeControllerCache {
            behavior: FakeFlushBehavior::Flushed,
        })),
        Some(Arc::new(FakeSystemCache {
            behavior: FakeFlushBehavior::Failed,
        })),
    );

    let failure = application
        .flush_with_id(
            crate::dns_cache_fixtures::test_core_scope(),
            DnsCacheOperationId(2),
        )
        .await
        .expect_err("adapter failure is not unsupported");

    assert_eq!(failure.code, ErrorCode::Network);
    assert_eq!(
        application.last_report().os_cache,
        DnsFlushOutcome::Failed { failure }
    );
}
