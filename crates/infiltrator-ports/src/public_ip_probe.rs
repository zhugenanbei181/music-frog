//! Typed port for one public-egress observation.
//!
//! The application owner stamps the request; the host adapter must echo that
//! stamp in a terminal answer. This keeps stale answers from a previous route
//! or a duplicate refresh out of the canonical readback.

use crate::runtime_http_proxy_endpoint::{RuntimeHttpProxyEndpoint, RuntimeHttpProxyEndpointPort};
use async_trait::async_trait;
use infiltrator_contract::public_ip_probe::{
    PublicIpObservation, PublicIpProbeAnswer, PublicIpProbeRequest, PublicIpProbeRouteStamp,
};
use infiltrator_contract::snapshot::PublicIpSnapshot;
use lurker_core_ports::error::PortError;
use std::sync::Arc;

/// Explicit route selected for one legacy host-level observation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PublicIpProbeRoute {
    Direct,
    ThroughHttpProxy { endpoint: String },
}

/// Legacy host-level network use-cases keep their existing adapter shape.
#[async_trait]
pub trait PublicIpProbe: Send + Sync {
    async fn probe(&self, route: PublicIpProbeRoute) -> Result<PublicIpSnapshot, PortError>;
}

/// Owner-scoped probe port. A concrete host adapter converts its transport
/// result into the stamped typed answer at the boundary.
#[async_trait]
pub trait PublicIpProbePort: Send + Sync {
    async fn observe(&self, request: PublicIpProbeRequest) -> PublicIpProbeAnswer;
}

/// Adapter from the legacy host `PublicIpProbe` + optional runtime proxy
/// endpoint into the owner-scoped typed port. The adapter never chooses a
/// route: the stamped request carries the owner's decision.
pub struct RoutedPublicIpProbePort {
    probe: Arc<dyn PublicIpProbe>,
    runtime_endpoint: Option<Arc<dyn RuntimeHttpProxyEndpointPort>>,
}

impl RoutedPublicIpProbePort {
    pub fn new(
        probe: Arc<dyn PublicIpProbe>,
        runtime_endpoint: Option<Arc<dyn RuntimeHttpProxyEndpointPort>>,
    ) -> Self {
        Self {
            probe,
            runtime_endpoint,
        }
    }
}

#[async_trait]
impl PublicIpProbePort for RoutedPublicIpProbePort {
    async fn observe(&self, request: PublicIpProbeRequest) -> PublicIpProbeAnswer {
        let route = match request.stamp.route {
            PublicIpProbeRouteStamp::Direct => PublicIpProbeRoute::Direct,
            PublicIpProbeRouteStamp::ThroughRuntimeHttpProxy => {
                let Some(runtime) = self.runtime_endpoint.as_ref() else {
                    return PublicIpProbeAnswer::Failed {
                        stamp: request.stamp,
                        reason: "runtime HTTP proxy endpoint owner is not composed".to_owned(),
                    };
                };
                match runtime.observe_http_proxy_endpoint().await {
                    Ok(RuntimeHttpProxyEndpoint::Configured { endpoint }) => {
                        PublicIpProbeRoute::ThroughHttpProxy { endpoint }
                    }
                    Ok(RuntimeHttpProxyEndpoint::NotConfigured) => {
                        return PublicIpProbeAnswer::Failed {
                            stamp: request.stamp,
                            reason: "no usable runtime HTTP proxy endpoint is configured"
                                .to_owned(),
                        };
                    }
                    Err(error) => {
                        return PublicIpProbeAnswer::Failed {
                            stamp: request.stamp,
                            reason: error.to_string(),
                        };
                    }
                }
            }
        };
        answer_from_snapshot(request, self.probe.probe(route).await)
    }
}

/// Convert a legacy neutral snapshot into the owner's typed answer.
pub fn answer_from_snapshot(
    request: PublicIpProbeRequest,
    result: Result<PublicIpSnapshot, PortError>,
) -> PublicIpProbeAnswer {
    match result {
        Ok(snapshot) => PublicIpProbeAnswer::Succeeded {
            stamp: request.stamp,
            observation: PublicIpObservation {
                ip: snapshot.ip,
                country: snapshot.country,
                region: snapshot.region,
                city: snapshot.city,
                provider: snapshot.provider,
                checked_at_epoch_ms: u64::try_from(snapshot.checked_at_epoch_ms)
                    .unwrap_or_default(),
            },
        },
        Err(error) => PublicIpProbeAnswer::Failed {
            stamp: request.stamp,
            reason: error.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::public_ip_probe::{PublicIpProbeRouteStamp, PublicIpProbeStamp};
    use std::sync::Mutex;

    #[test]
    fn a_host_error_is_a_typed_failed_answer() {
        let request = PublicIpProbeRequest::new(PublicIpProbeStamp {
            request_id: 4,
            core_generation: 2,
            revision: 9,
            route: PublicIpProbeRouteStamp::Direct,
        });
        let answer = answer_from_snapshot(request, Err(PortError::Network("offline".to_owned())));
        assert_eq!(answer.stamp(), request.stamp);
        assert!(matches!(answer, PublicIpProbeAnswer::Failed { .. }));
    }

    struct RecordingProbe {
        seen: Mutex<Vec<PublicIpProbeRoute>>,
    }

    #[async_trait]
    impl PublicIpProbe for RecordingProbe {
        async fn probe(&self, route: PublicIpProbeRoute) -> Result<PublicIpSnapshot, PortError> {
            self.seen.lock().expect("route lock").push(route.clone());
            Ok(PublicIpSnapshot {
                ip: "203.0.113.7".to_owned(),
                provider: "api.ipify.org".to_owned(),
                checked_at_epoch_ms: 1_741_300_000_000,
                country: None,
                region: None,
                city: None,
            })
        }
    }

    struct ConfiguredRuntime;

    #[async_trait]
    impl RuntimeHttpProxyEndpointPort for ConfiguredRuntime {
        async fn observe_http_proxy_endpoint(&self) -> Result<RuntimeHttpProxyEndpoint, PortError> {
            Ok(RuntimeHttpProxyEndpoint::Configured {
                endpoint: "127.0.0.1:7890".to_owned(),
            })
        }
    }

    #[tokio::test]
    async fn stamped_proxy_route_reaches_the_host_probe() {
        let probe = Arc::new(RecordingProbe {
            seen: Mutex::new(Vec::new()),
        });
        let port = RoutedPublicIpProbePort::new(probe.clone(), Some(Arc::new(ConfiguredRuntime)));
        let request = PublicIpProbeRequest::new(PublicIpProbeStamp {
            request_id: 7,
            core_generation: 3,
            revision: 42,
            route: PublicIpProbeRouteStamp::ThroughRuntimeHttpProxy,
        });
        let answer = port.observe(request).await;
        assert_eq!(answer.stamp(), request.stamp);
        assert!(matches!(answer, PublicIpProbeAnswer::Succeeded { .. }));
        let seen = probe.seen.lock().expect("route lock");
        assert_eq!(
            seen.as_slice(),
            &[PublicIpProbeRoute::ThroughHttpProxy {
                endpoint: "127.0.0.1:7890".to_owned()
            }]
        );
    }
}
