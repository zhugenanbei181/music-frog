//! Public-egress network use-cases over an injected probe port.

use infiltrator_contract::error::Failure;
use infiltrator_contract::snapshot::PublicIpSnapshot;
use infiltrator_ports::public_ip_probe::PublicIpProbe;
use infiltrator_ports::runtime_gateway::ManagedRuntime;
use std::sync::Arc;

#[derive(Clone)]
pub struct NetworkApplication {
    probe: Arc<dyn PublicIpProbe>,
}

impl NetworkApplication {
    pub fn new(probe: Arc<dyn PublicIpProbe>) -> Self {
        Self { probe }
    }

    pub async fn probe_public_ip(
        &self,
        proxy_endpoint: Option<String>,
    ) -> Result<PublicIpSnapshot, Failure> {
        self.probe
            .probe(proxy_endpoint)
            .await
            .map_err(Failure::from)
    }

    pub async fn probe_through_runtime<R: ManagedRuntime + ?Sized>(
        &self,
        runtime: Arc<R>,
    ) -> Result<PublicIpSnapshot, Failure> {
        let endpoint = runtime
            .http_proxy_endpoint()
            .await
            .map_err(Failure::from)?
            .ok_or_else(|| {
                Failure::new(
                    infiltrator_contract::error::ErrorCode::NotReady,
                    "no usable HTTP proxy endpoint is configured",
                    false,
                )
            })?;
        self.probe_public_ip(Some(endpoint)).await
    }
}
