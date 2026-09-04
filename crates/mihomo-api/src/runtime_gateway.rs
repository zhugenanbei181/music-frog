//! RuntimeGateway implementation for the HTTP controller client.

use async_trait::async_trait;
use infiltrator_contract::command::ProxyMode;
use infiltrator_domain::proxy::Proxy;
use infiltrator_domain::runtime::{
    ConfigSnapshot, ConnectionSnapshot, MemoryData, ProxyProvider, RuleProvider,
    TrafficData,
};
use infiltrator_ports::error::PortError;
use infiltrator_ports::runtime_gateway::{RuntimeGateway, RuntimeStream, RuntimeStreamEvent};
use serde_json::Value;
use std::collections::HashMap;

use crate::client::{MihomoClient, StreamEvent};

#[async_trait]
impl RuntimeGateway for MihomoClient {
    async fn get_config(&self) -> Result<ConfigSnapshot, PortError> {
        MihomoClient::get_config(self)
            .await
            .map(Into::into)
            .map_err(network_error)
    }

    async fn patch_config(&self, updates: Value) -> Result<(), PortError> {
        MihomoClient::patch_config(self, updates)
            .await
            .map_err(network_error)
    }

    async fn set_proxy_mode(&self, mode: ProxyMode) -> Result<(), PortError> {
        RuntimeGateway::patch_config(self, serde_json::json!({"mode": mode.to_wire()})).await
    }

    async fn get_proxies(&self) -> Result<HashMap<String, Proxy>, PortError> {
        MihomoClient::get_proxies(self)
            .await
            .map_err(network_error)
    }

    async fn switch_proxy(&self, group: &str, proxy: &str) -> Result<(), PortError> {
        MihomoClient::switch_proxy(self, group, proxy)
            .await
            .map_err(network_error)
    }

    async fn test_delay(
        &self,
        proxy: &str,
        url: &str,
        timeout_ms: u32,
    ) -> Result<u32, PortError> {
        MihomoClient::test_delay(self, proxy, url, timeout_ms)
            .await
            .map_err(network_error)
    }

    async fn get_proxy_providers(&self) -> Result<Vec<ProxyProvider>, PortError> {
        MihomoClient::get_proxy_providers(self)
            .await
            .map(|providers| providers.into_values().map(Into::into).collect())
            .map_err(network_error)
    }

    async fn get_rule_providers(&self) -> Result<Vec<RuleProvider>, PortError> {
        MihomoClient::get_rule_providers(self)
            .await
            .map(|providers| providers.into_values().map(Into::into).collect())
            .map_err(network_error)
    }

    async fn update_proxy_provider(&self, name: &str) -> Result<(), PortError> {
        MihomoClient::update_proxy_provider(self, name)
            .await
            .map_err(network_error)
    }

    async fn update_rule_provider(&self, name: &str) -> Result<(), PortError> {
        MihomoClient::update_rule_provider(self, name)
            .await
            .map_err(network_error)
    }

    async fn flush_fakeip_cache(&self) -> Result<(), PortError> {
        MihomoClient::flush_fakeip_cache(self)
            .await
            .map_err(network_error)
    }

    async fn get_connections(&self) -> Result<ConnectionSnapshot, PortError> {
        MihomoClient::get_connections(self)
            .await
            .map(Into::into)
            .map_err(network_error)
    }

    async fn get_memory(&self) -> Result<MemoryData, PortError> {
        MihomoClient::get_memory(self)
            .await
            .map(Into::into)
            .map_err(network_error)
    }

    async fn close_connection(&self, id: &str) -> Result<(), PortError> {
        MihomoClient::close_connection(self, id)
            .await
            .map_err(network_error)
    }

    async fn close_all_connections(&self) -> Result<(), PortError> {
        MihomoClient::close_all_connections(self)
            .await
            .map_err(network_error)
    }

    async fn stream_logs(&self, level: Option<String>) -> Result<RuntimeStream<String>, PortError> {
        let receiver = self
            .stream_logs_events(level.as_deref())
            .await
            .map_err(network_error)?;
        Ok(map_stream(receiver, |line| line))
    }

    async fn stream_traffic(&self) -> Result<RuntimeStream<TrafficData>, PortError> {
        let receiver = self
            .stream_traffic_events()
            .await
            .map_err(network_error)?;
        Ok(map_stream(receiver, Into::into))
    }

    async fn stream_connections(
        &self,
    ) -> Result<RuntimeStream<ConnectionSnapshot>, PortError> {
        let receiver = self
            .stream_connections_events()
            .await
            .map_err(network_error)?;
        Ok(map_stream(receiver, Into::into))
    }

}

fn network_error<E: std::fmt::Display>(error: E) -> PortError {
    PortError::Network(error.to_string())
}

fn map_stream<T, U>(
    receiver: tokio::sync::mpsc::UnboundedReceiver<StreamEvent<T>>,
    map_item: fn(T) -> U,
) -> RuntimeStream<U>
where
    T: Send + 'static,
    U: Send + 'static,
{
    Box::pin(futures_util::stream::unfold(
        receiver,
        move |mut receiver| async move {
            let event = receiver.recv().await?;
            let event = match event {
                StreamEvent::Connecting => RuntimeStreamEvent::Connecting,
                StreamEvent::Connected => RuntimeStreamEvent::Connected,
                StreamEvent::Item(item) => RuntimeStreamEvent::Item(map_item(item)),
                StreamEvent::Reconnecting(error) => RuntimeStreamEvent::Reconnecting(error),
                StreamEvent::Failed(error) => RuntimeStreamEvent::Failed(error),
            };
            Some((event, receiver))
        },
    ))
}
