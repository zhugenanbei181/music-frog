//! Public-egress network use-cases over an injected probe port.

use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::snapshot::PublicIpSnapshot;
use infiltrator_ports::public_ip_probe::{PublicIpProbe, PublicIpProbeRoute};
use infiltrator_ports::runtime_http_proxy_endpoint::{
    RuntimeHttpProxyEndpoint, RuntimeHttpProxyEndpointPort,
};
use std::sync::Arc;

#[derive(Clone)]
pub struct NetworkApplication {
    probe: Arc<dyn PublicIpProbe>,
}

impl NetworkApplication {
    pub fn new(probe: Arc<dyn PublicIpProbe>) -> Self {
        Self { probe }
    }

    pub async fn probe_public_ip_direct(&self) -> Result<PublicIpSnapshot, Failure> {
        self.probe
            .probe(PublicIpProbeRoute::Direct)
            .await
            .map_err(Failure::from)
    }

    pub async fn probe_public_ip_through_proxy(
        &self,
        endpoint: String,
    ) -> Result<PublicIpSnapshot, Failure> {
        self.probe
            .probe(PublicIpProbeRoute::ThroughHttpProxy { endpoint })
            .await
            .map_err(Failure::from)
    }

    pub async fn probe_through_runtime<R: RuntimeHttpProxyEndpointPort + ?Sized>(
        &self,
        runtime: Arc<R>,
    ) -> Result<PublicIpSnapshot, Failure> {
        let endpoint = match runtime
            .observe_http_proxy_endpoint()
            .await
            .map_err(Failure::from)?
        {
            RuntimeHttpProxyEndpoint::Configured { endpoint } => endpoint,
            RuntimeHttpProxyEndpoint::NotConfigured => {
                return Err(Failure::new(
                    ErrorCode::NotReady,
                    "no usable HTTP proxy endpoint is configured",
                    false,
                ));
            }
        };
        self.probe_public_ip_through_proxy(endpoint).await
    }
}

#[cfg(test)]
#[path = "network_application_tests.rs"]
mod tests;
