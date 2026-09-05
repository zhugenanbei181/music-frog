//! Runtime proxy use-cases over the controller port.

use futures_util::stream::{self, StreamExt};
use infiltrator_contract::error::Failure;
use infiltrator_domain::proxy::{ProxyGroup, ProxyHistory};
use infiltrator_ports::runtime_gateway::RuntimeGateway;
use std::sync::Arc;

/// A controller-neutral leaf proxy projection.
#[derive(Debug, Clone, PartialEq)]
pub struct ProxyNode {
    pub name: String,
    pub proxy_type: String,
    pub udp: bool,
    pub history: Vec<ProxyHistory>,
    pub delay: Option<u32>,
    pub alive: bool,
}

/// Proxy use-cases shared by CLI, Admin, and UI surfaces.
#[derive(Clone)]
pub struct ProxyApplication {
    gateway: Arc<dyn RuntimeGateway>,
}

impl ProxyApplication {
    pub fn new(gateway: Arc<dyn RuntimeGateway>) -> Self {
        Self { gateway }
    }

    pub async fn list_nodes(&self) -> Result<Vec<ProxyNode>, Failure> {
        let proxies = self.gateway.get_proxies().await.map_err(Failure::from)?;
        let mut nodes = proxies
            .into_iter()
            .filter_map(|(name, proxy)| {
                (!proxy.is_group()).then(|| ProxyNode {
                    name,
                    proxy_type: proxy.proxy_type().to_string(),
                    udp: proxy.udp(),
                    history: proxy.history().to_vec(),
                    delay: proxy.delay(),
                    alive: proxy.alive(),
                })
            })
            .collect::<Vec<_>>();
        nodes.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(nodes)
    }

    pub async fn list_groups(&self) -> Result<Vec<ProxyGroup>, Failure> {
        let proxies = self.gateway.get_proxies().await.map_err(Failure::from)?;
        let mut groups = proxies
            .into_iter()
            .filter_map(|(name, proxy)| {
                proxy.is_group().then(|| ProxyGroup {
                    name,
                    now: proxy.now().unwrap_or_default().to_string(),
                    all: proxy.all().unwrap_or_default().to_vec(),
                    history: proxy.history().to_vec(),
                })
            })
            .collect::<Vec<_>>();
        groups.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(groups)
    }

    pub async fn switch(&self, group: &str, proxy: &str) -> Result<(), Failure> {
        self.gateway
            .switch_proxy(group, proxy)
            .await
            .map_err(Failure::from)
    }

    pub async fn current(&self, group: &str) -> Result<String, Failure> {
        let proxies = self.gateway.get_proxies().await.map_err(Failure::from)?;
        let proxy = proxies.get(group).ok_or_else(|| {
            Failure::new(
                infiltrator_contract::error::ErrorCode::InvalidInput,
                format!("proxy group {group} was not found"),
                false,
            )
        })?;
        if !proxy.is_group() {
            return Err(Failure::new(
                infiltrator_contract::error::ErrorCode::InvalidInput,
                format!("{group} is not a proxy group"),
                false,
            ));
        }
        Ok(proxy.now().unwrap_or_default().to_string())
    }

    pub async fn test_delay(
        &self,
        proxy: &str,
        url: &str,
        timeout_ms: u32,
    ) -> Result<u32, Failure> {
        self.gateway
            .test_delay(proxy, url, timeout_ms)
            .await
            .map_err(Failure::from)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProxyDelayOutcome {
    pub proxy_name: String,
    pub result: Result<u32, String>,
}

/// Test a bounded set of proxies concurrently through the runtime gateway.
/// Timeout policy belongs to the gateway's typed `test_delay` operation, so
/// this use-case has no executor-specific timer or cancellation channel.
pub async fn test_proxy_delays<G: RuntimeGateway + ?Sized>(
    gateway: Arc<G>,
    proxies: Vec<String>,
    test_url: String,
    timeout_ms: u32,
    max_concurrency: usize,
) -> Vec<ProxyDelayOutcome> {
    stream::iter(proxies.into_iter().map(|proxy_name| {
        let gateway = Arc::clone(&gateway);
        let test_url = test_url.clone();
        async move {
            let result = gateway
                .test_delay(&proxy_name, &test_url, timeout_ms)
                .await
                .map_err(|error| error.to_string());
            ProxyDelayOutcome { proxy_name, result }
        }
    }))
    .buffer_unordered(max_concurrency.max(1))
    .collect()
    .await
}
