//! Runtime controller operations exposed as a transport-neutral port.

use async_trait::async_trait;
use futures_util::stream::BoxStream;
use infiltrator_domain::apply::ApplyStrategy;
use infiltrator_domain::proxy::Proxy;
use infiltrator_domain::runtime::{
    ConfigSnapshot, ConnectionSnapshot, MemoryData, ProxyProvider, RuleProvider,
};
use crate::error::PortError;
use std::collections::HashMap;

/// Lifecycle-independent events from a controller stream.
#[derive(Debug)]
pub enum RuntimeStreamEvent<T> {
    Connecting,
    Connected,
    Item(T),
    Reconnecting(String),
    Failed(String),
}

pub type RuntimeStream<T> = BoxStream<'static, RuntimeStreamEvent<T>>;

/// The controller-facing part of a running core host.
///
/// The trait is deliberately expressed only in domain values. It does not
/// expose `MihomoClient`, `reqwest`, Tokio receivers, process handles, or UI
/// task types. A desktop, Android, or iOS composition can provide its own
/// implementation without changing an inbound surface.
#[async_trait]
pub trait RuntimeGateway: Send + Sync {
    async fn get_config(&self) -> Result<ConfigSnapshot, PortError>;
    async fn patch_config(&self, updates: serde_json::Value) -> Result<(), PortError>;
    async fn set_proxy_mode(
        &self,
        mode: infiltrator_contract::command::ProxyMode,
    ) -> Result<(), PortError>;
    async fn get_proxies(&self) -> Result<HashMap<String, Proxy>, PortError>;
    async fn switch_proxy(&self, group: &str, proxy: &str) -> Result<(), PortError>;
    async fn test_delay(&self, proxy: &str, url: &str, timeout_ms: u32) -> Result<u32, PortError>;
    async fn get_proxy_providers(&self) -> Result<Vec<ProxyProvider>, PortError>;
    async fn get_rule_providers(&self) -> Result<Vec<RuleProvider>, PortError>;
    async fn update_proxy_provider(&self, name: &str) -> Result<(), PortError>;
    async fn update_rule_provider(&self, name: &str) -> Result<(), PortError>;
    async fn flush_fakeip_cache(&self) -> Result<(), PortError>;
    async fn get_connections(&self) -> Result<ConnectionSnapshot, PortError>;
    async fn get_memory(&self) -> Result<MemoryData, PortError>;
    async fn close_connection(&self, id: &str) -> Result<(), PortError>;
    async fn close_all_connections(&self) -> Result<(), PortError>;

    async fn stream_logs(&self, level: Option<String>) -> Result<RuntimeStream<String>, PortError>;
    async fn stream_traffic(
        &self,
    ) -> Result<RuntimeStream<infiltrator_domain::runtime::TrafficData>, PortError>;
    async fn stream_connections(
        &self,
    ) -> Result<RuntimeStream<infiltrator_domain::runtime::ConnectionSnapshot>, PortError>;
}

/// Additional host-owned operations available when a gateway also owns the
/// running core process and its configuration transaction.
#[async_trait]
pub trait ManagedRuntime: RuntimeGateway {
    fn generation(&self) -> u64;
    async fn is_running(&self) -> bool;
    async fn restart(&self) -> Result<u64, PortError>;
    async fn http_proxy_endpoint(&self) -> Result<Option<String>, PortError>;
    async fn shutdown(&self) -> Result<(), PortError>;
    async fn apply_current_config(&self, strategy: ApplyStrategy) -> Result<u64, PortError>;
    async fn apply_profile_content(
        &self,
        content: &str,
        strategy: ApplyStrategy,
    ) -> Result<u64, PortError>;
}
