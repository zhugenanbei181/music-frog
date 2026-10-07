//! Explicit isolated cache targets for behavior and captures; unrelated controller operations refuse.
use async_trait::async_trait;
use futures_util::future::pending;
use infiltrator_contract::capability::Capability;
use infiltrator_contract::command::ProxyMode;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_domain::proxy::Proxy;
use infiltrator_domain::runtime::{
    ConfigSnapshot, ConnectionSnapshot, MemoryData, ProxyProvider, RuleProvider, TrafficData,
};
use infiltrator_ports::error::PortError;
use infiltrator_ports::runtime_gateway::{RuntimeGateway, RuntimeStream};
use infiltrator_ports::system_dns_cache::SystemDnsCachePort;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CachePortMode {
    Allowed,
    Denied,
    Unsupported,
    Pending,
}
pub struct IsolatedCaches {
    pub fake_calls: AtomicUsize,
    pub system_calls: AtomicUsize,
    fake_mode: Mutex<CachePortMode>,
    system_mode: Mutex<CachePortMode>,
    fake_entries: Mutex<Vec<String>>,
    system_entries: Mutex<Vec<String>>,
}
impl Default for IsolatedCaches {
    fn default() -> Self {
        Self {
            fake_calls: AtomicUsize::new(0),
            system_calls: AtomicUsize::new(0),
            fake_mode: Mutex::new(CachePortMode::Allowed),
            system_mode: Mutex::new(CachePortMode::Allowed),
            fake_entries: Mutex::new(vec!["cached.example.test".into()]),
            system_entries: Mutex::new(vec!["host.example.test".into()]),
        }
    }
}
impl IsolatedCaches {
    pub fn set_fake_mode(&self, mode: CachePortMode) {
        *self.fake_mode.lock().expect("isolated Fake-IP mode") = mode;
    }
    pub fn set_system_mode(&self, mode: CachePortMode) {
        *self.system_mode.lock().expect("isolated OS mode") = mode;
    }
    pub fn contents(&self) -> (Vec<String>, Vec<String>) {
        (
            self.fake_entries
                .lock()
                .expect("isolated Fake-IP entries")
                .clone(),
            self.system_entries
                .lock()
                .expect("isolated OS entries")
                .clone(),
        )
    }
}
fn refused<T>() -> Result<T, PortError> {
    Err(PortError::unsupported(
        Capability::Dns,
        "unrelated controller operation refused by isolated cache fixture",
    ))
}
#[async_trait]
impl RuntimeGateway for IsolatedCaches {
    async fn get_config(&self) -> Result<ConfigSnapshot, PortError> {
        refused()
    }
    async fn patch_config(&self, _: Value) -> Result<(), PortError> {
        refused()
    }
    async fn set_proxy_mode(&self, _: ProxyMode) -> Result<(), PortError> {
        refused()
    }
    async fn get_proxies(&self) -> Result<HashMap<String, Proxy>, PortError> {
        refused()
    }
    async fn switch_proxy(&self, _: &str, _: &str) -> Result<(), PortError> {
        refused()
    }
    async fn test_delay(&self, _: &str, _: &str, _: u32) -> Result<u32, PortError> {
        refused()
    }
    async fn get_proxy_providers(&self) -> Result<Vec<ProxyProvider>, PortError> {
        refused()
    }
    async fn get_rule_providers(&self) -> Result<Vec<RuleProvider>, PortError> {
        refused()
    }
    async fn update_proxy_provider(&self, _: &str) -> Result<(), PortError> {
        refused()
    }
    async fn update_rule_provider(&self, _: &str) -> Result<(), PortError> {
        refused()
    }
    async fn flush_fakeip_cache(&self) -> Result<(), PortError> {
        self.fake_calls.fetch_add(1, Ordering::SeqCst);
        let mode = *self.fake_mode.lock().expect("isolated Fake-IP mode");
        match mode {
            CachePortMode::Allowed => {
                self.fake_entries
                    .lock()
                    .expect("isolated Fake-IP entries")
                    .clear();
                Ok(())
            }
            CachePortMode::Denied => Err(PortError::Rejected(Failure::new(
                ErrorCode::Permission,
                "Allow Fake-IP cache access, then retry clearing.",
                false,
            ))),
            CachePortMode::Unsupported => Err(PortError::unsupported(
                Capability::Dns,
                "this controller has no Fake-IP flush",
            )),
            CachePortMode::Pending => pending().await,
        }
    }
    async fn get_connections(&self) -> Result<ConnectionSnapshot, PortError> {
        refused()
    }
    async fn get_memory(&self) -> Result<MemoryData, PortError> {
        refused()
    }
    async fn close_connection(&self, _: &str) -> Result<(), PortError> {
        refused()
    }
    async fn close_all_connections(&self) -> Result<(), PortError> {
        refused()
    }
    async fn stream_logs(&self, _: Option<String>) -> Result<RuntimeStream<String>, PortError> {
        refused()
    }
    async fn stream_traffic(&self) -> Result<RuntimeStream<TrafficData>, PortError> {
        refused()
    }
    async fn stream_connections(&self) -> Result<RuntimeStream<ConnectionSnapshot>, PortError> {
        refused()
    }
}
#[async_trait]
impl SystemDnsCachePort for IsolatedCaches {
    async fn flush_system_cache(&self) -> Result<bool, PortError> {
        self.system_calls.fetch_add(1, Ordering::SeqCst);
        let mode = *self.system_mode.lock().expect("isolated OS mode");
        match mode {
            CachePortMode::Allowed => {
                self.system_entries
                    .lock()
                    .expect("isolated OS entries")
                    .clear();
                Ok(true)
            }
            CachePortMode::Denied => Err(PortError::Rejected(Failure::new(
                ErrorCode::Permission,
                "Allow system resolver cache access, then retry clearing.",
                false,
            ))),
            CachePortMode::Unsupported => Ok(false),
            CachePortMode::Pending => pending().await,
        }
    }
}
