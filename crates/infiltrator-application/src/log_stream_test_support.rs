//! test-intent: support
use async_trait::async_trait;
use futures_util::stream::pending;
use infiltrator_contract::command::ProxyMode;
use infiltrator_domain::proxy::Proxy;
use infiltrator_domain::runtime::{
    ConfigSnapshot, ConnectionSnapshot, MemoryData, ProxyProvider, RuleProvider, TrafficData,
};
use infiltrator_ports::application_runtime::{
    ApplicationFuture, ApplicationRuntime, ApplicationSleep,
};
use infiltrator_ports::error::PortError;
use infiltrator_ports::runtime_gateway::{RuntimeGateway, RuntimeStream};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::runtime::Builder;
use tokio::sync::Notify;
use tokio::time::sleep;

pub struct TestRuntime;
impl ApplicationRuntime for TestRuntime {
    fn block_on(&self, future: ApplicationFuture) {
        Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(future);
    }
    fn sleep(&self, duration: Duration) -> ApplicationSleep<'_> {
        Box::pin(sleep(duration))
    }
}
#[derive(Default)]
pub struct LogGateway {
    pub streams: Mutex<Vec<RuntimeStream<String>>>,
    pub calls: AtomicUsize,
    pub opened: Arc<Notify>,
    pub opening_gate: Option<Arc<Notify>>,
    pub failure: Option<PortError>,
}
#[async_trait]
impl RuntimeGateway for LogGateway {
    async fn stream_logs(&self, level: Option<String>) -> Result<RuntimeStream<String>, PortError> {
        assert_eq!(level.as_deref(), Some("debug"));
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.opened.notify_one();
        if let Some(gate) = &self.opening_gate {
            gate.notified().await;
        }
        if let Some(failure) = &self.failure {
            return Err(failure.clone());
        }
        let mut streams = self.streams.lock().unwrap();
        if streams.is_empty() {
            Ok(Box::pin(pending()))
        } else {
            Ok(streams.remove(0))
        }
    }
    async fn get_config(&self) -> Result<ConfigSnapshot, PortError> {
        panic!("unexpected config read")
    }
    async fn patch_config(&self, _: serde_json::Value) -> Result<(), PortError> {
        panic!("unexpected config write")
    }
    async fn set_proxy_mode(&self, _: ProxyMode) -> Result<(), PortError> {
        panic!("unexpected mode write")
    }
    async fn get_proxies(&self) -> Result<HashMap<String, Proxy>, PortError> {
        panic!("unexpected proxy read")
    }
    async fn switch_proxy(&self, _: &str, _: &str) -> Result<(), PortError> {
        panic!("unexpected proxy write")
    }
    async fn test_delay(&self, _: &str, _: &str, _: u32) -> Result<u32, PortError> {
        panic!("unexpected probe")
    }
    async fn get_proxy_providers(&self) -> Result<Vec<ProxyProvider>, PortError> {
        panic!("unexpected provider read")
    }
    async fn get_rule_providers(&self) -> Result<Vec<RuleProvider>, PortError> {
        panic!("unexpected provider read")
    }
    async fn update_proxy_provider(&self, _: &str) -> Result<(), PortError> {
        panic!("unexpected provider write")
    }
    async fn update_rule_provider(&self, _: &str) -> Result<(), PortError> {
        panic!("unexpected provider write")
    }
    async fn flush_fakeip_cache(&self) -> Result<(), PortError> {
        panic!("unexpected cache write")
    }
    async fn get_connections(&self) -> Result<ConnectionSnapshot, PortError> {
        panic!("unexpected connection read")
    }
    async fn get_memory(&self) -> Result<MemoryData, PortError> {
        panic!("unexpected memory read")
    }
    async fn close_connection(&self, _: &str) -> Result<(), PortError> {
        panic!("unexpected connection write")
    }
    async fn close_all_connections(&self) -> Result<(), PortError> {
        panic!("unexpected connection write")
    }
    async fn stream_traffic(&self) -> Result<RuntimeStream<TrafficData>, PortError> {
        panic!("unexpected traffic stream")
    }
    async fn stream_connections(&self) -> Result<RuntimeStream<ConnectionSnapshot>, PortError> {
        panic!("unexpected connection stream")
    }
}
