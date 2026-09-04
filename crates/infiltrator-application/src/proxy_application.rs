//! Runtime proxy use-cases over the controller port.

use futures_util::stream::{self, StreamExt};
use infiltrator_ports::runtime_gateway::RuntimeGateway;
use std::sync::Arc;

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
