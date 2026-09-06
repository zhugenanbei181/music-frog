//! Runtime-neutral physical-link roaming and TUN route recovery use-case.

use futures_util::lock::Mutex;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::network_roaming::{
    NetworkInterfaceKind, NetworkObservation, NetworkRoamingEvent, NetworkRoamingRepairRequest,
    NetworkRoamingSnapshot, NetworkRoamingStatus,
};
use infiltrator_ports::network_roaming::NetworkRoamingPort;
use infiltrator_ports::runtime_gateway::RuntimeGateway;
use std::sync::Arc;

struct State {
    last_observation: Option<NetworkObservation>,
    snapshot: NetworkRoamingSnapshot,
    next_revision: u64,
    route_repair_count: u64,
}

/// Application owner of network observation, migration detection and recovery.
/// The UI only invokes `refresh`/`force_repair` and renders the returned
/// contract snapshot.
#[derive(Clone)]
pub struct NetworkRoamingApplication {
    port: Arc<dyn NetworkRoamingPort>,
    gateway: Option<Arc<dyn RuntimeGateway>>,
    state: Arc<Mutex<State>>,
}

impl NetworkRoamingApplication {
    pub fn new(
        port: Arc<dyn NetworkRoamingPort>,
        gateway: Option<Arc<dyn RuntimeGateway>>,
    ) -> Self {
        Self {
            port,
            gateway,
            state: Arc::new(Mutex::new(State {
                last_observation: None,
                snapshot: NetworkRoamingSnapshot::default(),
                next_revision: 1,
                route_repair_count: 0,
            })),
        }
    }

    pub async fn snapshot(&self) -> NetworkRoamingSnapshot {
        self.state.lock().await.snapshot.clone()
    }

    /// Observe the host and automatically repair a changed physical route when
    /// Mihomo TUN auto-route is live. A failed repair is retained as a typed
    /// failed snapshot and never becomes a success with stale data.
    pub async fn refresh(&self) -> NetworkRoamingSnapshot {
        match self.reconcile(false).await {
            Ok(snapshot) => snapshot,
            Err(_) => self.snapshot().await,
        }
    }

    /// Refresh using a caller's last shared projection as the migration
    /// baseline. This keeps one-shot surface actions (such as a manual Iced
    /// refresh) behaviorally equivalent to the long-lived surface reader.
    pub async fn refresh_from(
        &self,
        baseline: Option<NetworkRoamingSnapshot>,
    ) -> NetworkRoamingSnapshot {
        if let Some(baseline) = baseline.filter(|snapshot| !snapshot.interfaces.is_empty()) {
            let mut state = self.state.lock().await;
            state.last_observation = Some(NetworkObservation {
                interfaces: baseline.interfaces.clone(),
                observed_at_epoch_ms: baseline.observed_at_epoch_ms,
            });
            state.next_revision = baseline.revision.saturating_add(1).max(1);
            state.route_repair_count = baseline.route_repair_count;
            state.snapshot = baseline;
        }
        self.refresh().await
    }

    /// Force a route repair using the current live gateway, even when no
    /// migration has been observed since the last sample.
    pub async fn force_repair(&self) -> Result<NetworkRoamingSnapshot, Failure> {
        self.reconcile(true).await
    }

    async fn reconcile(&self, force: bool) -> Result<NetworkRoamingSnapshot, Failure> {
        let mut state = self.state.lock().await;
        let revision = state.next_revision;
        state.next_revision = state.next_revision.saturating_add(1);

        let observation = match self.port.observe().await {
            Ok(observation) => observation,
            Err(error) => {
                let failure = Failure::from(error);
                state.snapshot = if failure.code == ErrorCode::Unsupported {
                    NetworkRoamingSnapshot::unsupported(revision, failure.message.clone())
                } else {
                    NetworkRoamingSnapshot::failed(revision, failure.clone())
                };
                return Err(failure);
            }
        };

        let (tun, controller_failure) = match &self.gateway {
            Some(gateway) => match gateway.get_config().await {
                Ok(config) => (config.tun, None),
                Err(error) => (
                    None,
                    Some(Failure::new(
                        ErrorCode::Network,
                        format!("Mihomo TUN configuration unavailable: {error}"),
                        true,
                    )),
                ),
            },
            None => (None, None),
        };
        let tun_interface = observation
            .interfaces
            .iter()
            .find(|interface| interface.is_up && interface.kind == NetworkInterfaceKind::Tun)
            .map(|interface| interface.name.clone());
        let decision = infiltrator_domain::network_roaming::decide(
            state.last_observation.as_ref(),
            &observation,
            tun.as_ref(),
            tun_interface.as_deref(),
        );
        let selected =
            infiltrator_domain::network_roaming::select_active_interface(&observation.interfaces);
        let (physical_mtu, recommended_tun_mtu, tcp_mss) = selected
            .and_then(|interface| interface.mtu)
            .map(|mtu| {
                let (tun_mtu, tcp_mss) =
                    infiltrator_domain::mtu_optimizer::MtuOptimizer::negotiate_tun_mtu(mtu);
                (Some(mtu), Some(tun_mtu), Some(tcp_mss))
            })
            .unwrap_or_default();
        let mut snapshot = NetworkRoamingSnapshot {
            status: if selected.is_some() {
                NetworkRoamingStatus::Stable
            } else {
                NetworkRoamingStatus::Degraded {
                    reason: decision.reason.clone(),
                }
            },
            interfaces: observation.interfaces.clone(),
            active_interface: decision.active_interface.clone(),
            default_gateway: decision.gateway_ip.clone(),
            previous_interface: decision.previous_interface.clone(),
            tun_interface: tun_interface.clone(),
            physical_mtu,
            recommended_tun_mtu,
            tcp_mss,
            route_repair_count: state.route_repair_count,
            last_event: state.snapshot.last_event.clone(),
            observed_at_epoch_ms: observation.observed_at_epoch_ms,
            revision,
        };
        if let Some(failure) = controller_failure.as_ref() {
            snapshot.status = NetworkRoamingStatus::Degraded {
                reason: failure.message.clone(),
            };
        }

        if state.last_observation.is_none() {
            snapshot.last_event = Some(NetworkRoamingEvent::InitialObservation {
                interface: decision.active_interface.clone(),
                gateway_ip: decision.gateway_ip.clone(),
            });
        } else if decision.changed {
            snapshot.last_event = if decision.active_address_changed
                && decision.previous_interface == decision.active_interface
                && decision.previous_gateway_ip == decision.gateway_ip
            {
                decision
                    .active_interface
                    .clone()
                    .map(|interface| NetworkRoamingEvent::InterfaceAddressChanged { interface })
            } else {
                Some(NetworkRoamingEvent::GatewayChanged {
                    old_interface: decision.previous_interface.clone(),
                    new_interface: decision.active_interface.clone(),
                    old_gateway_ip: decision.previous_gateway_ip.clone(),
                    new_gateway_ip: decision.gateway_ip.clone(),
                })
            };
        }

        if force && selected.is_none() {
            let failure = Failure::new(
                ErrorCode::NotReady,
                "no live physical egress interface is available",
                true,
            );
            snapshot.status = NetworkRoamingStatus::Failed {
                failure: failure.clone(),
            };
            snapshot.last_event = Some(NetworkRoamingEvent::RepairFailed {
                failure: failure.clone(),
            });
            state.snapshot = snapshot;
            return Err(failure);
        }

        let should_repair = selected.is_some() && (force || decision.route_repair_required);
        if should_repair {
            let failure = if self.gateway.is_none() {
                Some(Failure::unsupported(
                    "Mihomo gateway is not composed for network route repair",
                ))
            } else if let Some(failure) = controller_failure.clone() {
                Some(failure)
            } else if tun.as_ref().is_none_or(|tun| !tun.enable) {
                Some(Failure::new(
                    ErrorCode::NotReady,
                    "TUN is not enabled; no route table needs repair",
                    false,
                ))
            } else if tun.as_ref().is_none_or(|tun| !tun.auto_route) {
                Some(Failure::new(
                    ErrorCode::InvalidState,
                    "TUN auto-route is disabled; refusing to mutate host routes",
                    false,
                ))
            } else if tun_interface.is_none() {
                Some(Failure::new(
                    ErrorCode::NotReady,
                    "active TUN interface was not observed",
                    true,
                ))
            } else {
                None
            };
            if let Some(failure) = failure {
                snapshot.status = NetworkRoamingStatus::Failed {
                    failure: failure.clone(),
                };
                snapshot.last_event = Some(NetworkRoamingEvent::RepairFailed {
                    failure: failure.clone(),
                });
                state.snapshot = snapshot;
                return Err(failure);
            }

            let selected = selected.expect("selected interface checked above");
            let tun = tun.as_ref().expect("TUN checked above");
            let request = NetworkRoamingRepairRequest {
                physical_interface: selected.name.clone(),
                gateway_ip: selected.gateway_ip.clone(),
                tun_interface: tun_interface.clone().expect("TUN checked above"),
                strict_route: tun.strict_route,
            };
            snapshot.status = NetworkRoamingStatus::Recovering;
            match self.port.repair(request.clone()).await {
                Ok(result) => {
                    state.route_repair_count = state.route_repair_count.saturating_add(1);
                    snapshot.route_repair_count = state.route_repair_count;
                    snapshot.status = NetworkRoamingStatus::Stable;
                    snapshot.last_event = Some(NetworkRoamingEvent::RoutesRepaired {
                        physical_interface: request.physical_interface,
                        tun_interface: request.tun_interface,
                        detail: result.detail,
                    });
                }
                Err(error) => {
                    let failure = Failure::from(error);
                    snapshot.status = NetworkRoamingStatus::Failed {
                        failure: failure.clone(),
                    };
                    snapshot.last_event = Some(NetworkRoamingEvent::RepairFailed {
                        failure: failure.clone(),
                    });
                    state.snapshot = snapshot;
                    return Err(failure);
                }
            }
        } else if decision.changed {
            snapshot.last_event = Some(NetworkRoamingEvent::RepairSkipped {
                reason: controller_failure
                    .as_ref()
                    .map(|failure| failure.message.clone())
                    .unwrap_or_else(|| decision.reason.clone()),
            });
            if let Some(failure) = controller_failure {
                snapshot.status = NetworkRoamingStatus::Degraded {
                    reason: failure.message,
                };
            } else if tun.as_ref().is_some_and(|tun| tun.enable && tun.auto_route)
                && tun_interface.is_none()
            {
                snapshot.status = NetworkRoamingStatus::Degraded {
                    reason: "active TUN route was configured but no TUN interface was observed"
                        .to_owned(),
                };
            }
        }

        state.last_observation = Some(observation);
        state.snapshot = snapshot.clone();
        Ok(snapshot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use infiltrator_contract::network_roaming::{
        NetworkInterfaceKind, NetworkInterfaceSnapshot, NetworkObservation,
        NetworkRoamingRepairResult,
    };
    use infiltrator_domain::proxy::Proxy;
    use infiltrator_domain::runtime::{
        ConfigSnapshot, ConnectionSnapshot, MemoryData, ProxyProvider, RuleProvider, TrafficData,
    };
    use infiltrator_ports::error::PortError;
    use infiltrator_ports::runtime_gateway::{RuntimeGateway, RuntimeStream, RuntimeStreamEvent};
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct FakePort {
        observations: std::sync::Mutex<Vec<NetworkObservation>>,
        repairs: AtomicUsize,
    }

    #[async_trait]
    impl NetworkRoamingPort for FakePort {
        async fn observe(&self) -> Result<NetworkObservation, PortError> {
            let mut observations = self.observations.lock().expect("observations lock");
            if observations.len() > 1 {
                Ok(observations.remove(0))
            } else {
                Ok(observations.first().cloned().expect("observation fixture"))
            }
        }

        async fn repair(
            &self,
            _request: NetworkRoamingRepairRequest,
        ) -> Result<NetworkRoamingRepairResult, PortError> {
            self.repairs.fetch_add(1, Ordering::SeqCst);
            Ok(NetworkRoamingRepairResult {
                route_generation: 2,
                detail: "test route readback matched".to_owned(),
            })
        }
    }

    struct FakeGateway {
        auto_route: bool,
    }

    #[async_trait]
    impl RuntimeGateway for FakeGateway {
        async fn get_config(&self) -> Result<ConfigSnapshot, PortError> {
            Ok(ConfigSnapshot {
                tun: Some(infiltrator_domain::runtime::TunSnapshot {
                    enable: true,
                    stack: "gvisor".to_owned(),
                    auto_route: self.auto_route,
                    strict_route: true,
                    mtu: Some(1420),
                }),
                ..ConfigSnapshot::default()
            })
        }

        async fn patch_config(&self, _updates: serde_json::Value) -> Result<(), PortError> {
            Ok(())
        }

        async fn set_proxy_mode(
            &self,
            _mode: infiltrator_contract::command::ProxyMode,
        ) -> Result<(), PortError> {
            Ok(())
        }

        async fn get_proxies(&self) -> Result<HashMap<String, Proxy>, PortError> {
            Ok(HashMap::new())
        }

        async fn switch_proxy(&self, _group: &str, _node: &str) -> Result<(), PortError> {
            Ok(())
        }

        async fn test_delay(
            &self,
            _proxy: &str,
            _url: &str,
            _timeout_ms: u32,
        ) -> Result<u32, PortError> {
            Ok(0)
        }

        async fn get_proxy_providers(&self) -> Result<Vec<ProxyProvider>, PortError> {
            Ok(Vec::new())
        }

        async fn get_rule_providers(&self) -> Result<Vec<RuleProvider>, PortError> {
            Ok(Vec::new())
        }

        async fn update_proxy_provider(&self, _name: &str) -> Result<(), PortError> {
            Ok(())
        }

        async fn update_rule_provider(&self, _name: &str) -> Result<(), PortError> {
            Ok(())
        }

        async fn flush_fakeip_cache(&self) -> Result<(), PortError> {
            Ok(())
        }

        async fn get_connections(&self) -> Result<ConnectionSnapshot, PortError> {
            Ok(ConnectionSnapshot::default())
        }

        async fn get_memory(&self) -> Result<MemoryData, PortError> {
            Ok(MemoryData::default())
        }

        async fn close_connection(&self, _id: &str) -> Result<(), PortError> {
            Ok(())
        }

        async fn close_all_connections(&self) -> Result<(), PortError> {
            Ok(())
        }

        async fn stream_logs(
            &self,
            _level: Option<String>,
        ) -> Result<RuntimeStream<String>, PortError> {
            Ok(Box::pin(futures_util::stream::empty::<
                RuntimeStreamEvent<String>,
            >()))
        }

        async fn stream_traffic(&self) -> Result<RuntimeStream<TrafficData>, PortError> {
            Ok(Box::pin(futures_util::stream::empty::<
                RuntimeStreamEvent<TrafficData>,
            >()))
        }

        async fn stream_connections(&self) -> Result<RuntimeStream<ConnectionSnapshot>, PortError> {
            Ok(Box::pin(futures_util::stream::empty::<
                RuntimeStreamEvent<ConnectionSnapshot>,
            >()))
        }
    }

    fn observation(physical: &str, gateway: &str) -> NetworkObservation {
        NetworkObservation {
            interfaces: vec![
                NetworkInterfaceSnapshot {
                    name: physical.to_owned(),
                    kind: NetworkInterfaceKind::Ethernet,
                    is_up: true,
                    is_default_gateway: true,
                    gateway_ip: Some(gateway.to_owned()),
                    ip_addresses: vec![format!("{physical}-ip")],
                    mtu: Some(1500),
                    metric: Some(100),
                    dns_servers: Vec::new(),
                },
                NetworkInterfaceSnapshot {
                    name: "Meta".to_owned(),
                    kind: NetworkInterfaceKind::Tun,
                    is_up: true,
                    ..NetworkInterfaceSnapshot::default()
                },
            ],
            observed_at_epoch_ms: Some(1),
        }
    }

    #[tokio::test]
    async fn migration_repairs_routes_and_publishes_a_shared_snapshot() {
        let port = Arc::new(FakePort {
            observations: std::sync::Mutex::new(vec![
                observation("wlan0", "192.168.2.1"),
                observation("eth0", "192.168.1.1"),
            ]),
            repairs: AtomicUsize::new(0),
        });
        let application = NetworkRoamingApplication::new(
            port.clone(),
            Some(Arc::new(FakeGateway { auto_route: true })),
        );
        let first = application.refresh().await;
        assert_eq!(first.status, NetworkRoamingStatus::Stable);
        let second = application.refresh().await;
        assert_eq!(second.status, NetworkRoamingStatus::Stable);
        assert_eq!(second.active_interface.as_deref(), Some("eth0"));
        assert_eq!(second.route_repair_count, 1);
        assert!(matches!(
            second.last_event,
            Some(NetworkRoamingEvent::RoutesRepaired { .. })
        ));
        assert_eq!(port.repairs.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn migration_is_projected_without_host_mutation_when_auto_route_is_off() {
        let port = Arc::new(FakePort {
            observations: std::sync::Mutex::new(vec![
                observation("wlan0", "192.168.2.1"),
                observation("eth0", "192.168.1.1"),
            ]),
            repairs: AtomicUsize::new(0),
        });
        let application = NetworkRoamingApplication::new(
            port.clone(),
            Some(Arc::new(FakeGateway { auto_route: false })),
        );
        let _ = application.refresh().await;
        let snapshot = application.refresh().await;
        assert_eq!(snapshot.active_interface.as_deref(), Some("eth0"));
        assert_eq!(snapshot.route_repair_count, 0);
        assert!(matches!(
            snapshot.last_event,
            Some(NetworkRoamingEvent::RepairSkipped { .. })
        ));
        assert_eq!(port.repairs.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn force_repair_requires_a_composed_live_gateway() {
        let port = Arc::new(FakePort {
            observations: std::sync::Mutex::new(vec![observation("eth0", "192.168.1.1")]),
            repairs: AtomicUsize::new(0),
        });
        let application = NetworkRoamingApplication::new(port, None);
        let failure = application
            .force_repair()
            .await
            .expect_err("route repair without a gateway must fail closed");
        assert_eq!(failure.code, ErrorCode::Unsupported);
    }
}
