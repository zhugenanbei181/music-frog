use super::*;
use crate::resource_application::ResourceApplication;
use async_trait::async_trait;
use futures_util::stream::empty;
use infiltrator_contract::command::CoreLogLevel;
use infiltrator_contract::resources::{CORE_MEMORY_SOFT_LIMIT_BYTES, CoreGcStatus};
use infiltrator_domain::runtime::{
    ConnectionSnapshot, MemoryData, ProxyProvider, RuleProvider, TrafficData,
};
use infiltrator_ports::core_log_level::CoreLogLevelPort;
use infiltrator_ports::error::PortError;
use infiltrator_ports::runtime_gateway::{RuntimeGateway, RuntimeStream, RuntimeStreamEvent};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

struct TestGateway {
    level: Mutex<String>,
    memory_bytes: Mutex<u64>,
    gc_calls: AtomicUsize,
    ignore_log_level_write: bool,
}

#[async_trait]
impl RuntimeGateway for TestGateway {
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

    async fn get_connections(&self) -> Result<ConnectionSnapshot, PortError> {
        Ok(ConnectionSnapshot::default())
    }

    async fn get_memory(&self) -> Result<MemoryData, PortError> {
        Ok(MemoryData {
            in_use: *self.memory_bytes.lock().expect("memory lock"),
            os_limit: 0,
        })
    }

    async fn trigger_gc(&self) -> Result<(), PortError> {
        self.gc_calls.fetch_add(1, Ordering::SeqCst);
        *self.memory_bytes.lock().expect("memory lock") = 400 * 1024 * 1024;
        Ok(())
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
        Ok(Box::pin(empty::<RuntimeStreamEvent<String>>()))
    }

    async fn stream_traffic(&self) -> Result<RuntimeStream<TrafficData>, PortError> {
        Ok(Box::pin(empty::<RuntimeStreamEvent<TrafficData>>()))
    }

    async fn stream_connections(&self) -> Result<RuntimeStream<ConnectionSnapshot>, PortError> {
        Ok(Box::pin(empty::<RuntimeStreamEvent<ConnectionSnapshot>>()))
    }
}

#[async_trait]
impl CoreLogLevelPort for TestGateway {
    async fn set_core_log_level(&self, level: CoreLogLevel) -> Result<CoreLogLevel, PortError> {
        if !self.ignore_log_level_write {
            *self.level.lock().expect("level lock") = level.as_key().to_owned();
        }
        let observed = self.level.lock().expect("level lock").clone();
        CoreLogLevel::from_key(&observed)
            .ok_or_else(|| PortError::Failed(format!("unknown test log level: {observed}")))
    }
}

fn gateway(memory_bytes: u64, ignore_log_level_write: bool) -> Arc<TestGateway> {
    Arc::new(TestGateway {
        level: Mutex::new("info".to_owned()),
        memory_bytes: Mutex::new(memory_bytes),
        gc_calls: AtomicUsize::new(0),
        ignore_log_level_write,
    })
}

#[tokio::test]
async fn core_log_level_uses_the_typed_port_and_reports_its_readback() {
    let gateway = gateway(0, false);
    RuntimeQueryApplication::new(gateway.clone())
        .with_core_log_level_port(gateway.clone())
        .set_core_log_level(CoreLogLevel::Debug)
        .await
        .expect("log-level readback should match");
    assert_eq!(gateway.level.lock().expect("level lock").as_str(), "debug");
}

#[tokio::test]
async fn core_log_level_readback_mismatch_is_not_reported_as_success() {
    let gateway = gateway(0, true);
    let failure = RuntimeQueryApplication::new(gateway.clone())
        .with_core_log_level_port(gateway)
        .set_core_log_level(CoreLogLevel::Debug)
        .await
        .expect_err("ignored update must not report success");
    assert_eq!(
        failure.code,
        infiltrator_contract::error::ErrorCode::InvalidState
    );
}

#[tokio::test]
async fn resource_poll_triggers_gc_once_when_memory_exceeds_soft_limit() {
    let gateway = gateway(CORE_MEMORY_SOFT_LIMIT_BYTES + 1, false);
    let application = ResourceApplication::new(gateway.clone());
    let first = application.poll().await.expect("first poll");
    assert!(matches!(first.gc, CoreGcStatus::Triggered { .. }));
    assert_eq!(gateway.gc_calls.load(Ordering::SeqCst), 1);

    let second = application.poll().await.expect("second poll");
    assert!(matches!(second.gc, CoreGcStatus::NotNeeded));
    assert_eq!(gateway.gc_calls.load(Ordering::SeqCst), 1);
}
