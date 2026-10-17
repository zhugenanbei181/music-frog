use super::NetworkApplication;
use async_trait::async_trait;
use infiltrator_contract::snapshot::PublicIpSnapshot;
use infiltrator_ports::public_ip_probe::{PublicIpProbe, PublicIpProbeRoute};
use infiltrator_ports::runtime_http_proxy_endpoint::{
    RuntimeHttpProxyEndpoint, RuntimeHttpProxyEndpointPort,
};
use lurker_core_contract::error::ErrorCode;
use lurker_core_ports::error::PortError;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct ProbeFake {
    routes: Mutex<Vec<PublicIpProbeRoute>>,
}

#[async_trait]
impl PublicIpProbe for ProbeFake {
    async fn probe(&self, route: PublicIpProbeRoute) -> Result<PublicIpSnapshot, PortError> {
        self.routes.lock().expect("probe routes").push(route);
        Ok(PublicIpSnapshot {
            ip: "192.0.2.1".to_owned(),
            provider: "test".to_owned(),
            checked_at_epoch_ms: 1,
            country: None,
            region: None,
            city: None,
        })
    }
}

struct EndpointFake(RuntimeHttpProxyEndpoint);

#[async_trait]
impl RuntimeHttpProxyEndpointPort for EndpointFake {
    async fn observe_http_proxy_endpoint(&self) -> Result<RuntimeHttpProxyEndpoint, PortError> {
        Ok(self.0.clone())
    }
}

#[tokio::test]
async fn direct_probe_uses_an_explicit_direct_route() {
    let probe = Arc::new(ProbeFake::default());
    let application = NetworkApplication::new(Arc::clone(&probe) as Arc<dyn PublicIpProbe>);

    application.probe_public_ip_direct().await.unwrap();

    assert_eq!(
        *probe.routes.lock().expect("probe routes"),
        [PublicIpProbeRoute::Direct]
    );
}

#[tokio::test]
async fn runtime_probe_uses_only_a_configured_typed_endpoint() {
    let probe = Arc::new(ProbeFake::default());
    let application = NetworkApplication::new(Arc::clone(&probe) as Arc<dyn PublicIpProbe>);
    let endpoint = Arc::new(EndpointFake(RuntimeHttpProxyEndpoint::Configured {
        endpoint: "127.0.0.1:7890".to_owned(),
    }));

    application.probe_through_runtime(endpoint).await.unwrap();

    assert_eq!(
        *probe.routes.lock().expect("probe routes"),
        [PublicIpProbeRoute::ThroughHttpProxy {
            endpoint: "127.0.0.1:7890".to_owned(),
        }]
    );
}

#[tokio::test]
async fn runtime_probe_rejects_an_unconfigured_endpoint_before_probing() {
    let probe = Arc::new(ProbeFake::default());
    let application = NetworkApplication::new(Arc::clone(&probe) as Arc<dyn PublicIpProbe>);
    let endpoint = Arc::new(EndpointFake(RuntimeHttpProxyEndpoint::NotConfigured));

    let failure = application
        .probe_through_runtime(endpoint)
        .await
        .expect_err("an unconfigured endpoint must not become a direct probe");

    assert_eq!(failure.code, ErrorCode::NotReady);
    assert!(probe.routes.lock().expect("probe routes").is_empty());
}
