//! Runtime-neutral physical-link roaming and TUN route recovery use-case.

use futures_util::lock::Mutex;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::network_roaming::{
    NetworkInterfaceKind, NetworkObservation, NetworkRoamingEvent, NetworkRoamingSnapshot,
    NetworkRoamingStatus,
};
use infiltrator_domain::mtu_optimizer::MtuOptimizer;
use infiltrator_domain::network_roaming::{
    NetworkRoamingBaseline, NetworkRoamingDecision, decide, select_active_interface,
};
use infiltrator_ports::network_roaming::NetworkRoamingPort;
use infiltrator_ports::runtime_gateway::RuntimeGateway;
use std::sync::Arc;

mod route_repair;

use route_repair::{
    ReconcileIntent, RouteRepairPlan, TunConfigurationObservation, plan_route_repair,
};

struct State {
    observation_history: NetworkObservationHistory,
    snapshot: NetworkRoamingSnapshot,
    next_revision: u64,
    route_repair_count: u64,
}

enum NetworkObservationHistory {
    AwaitingInitial,
    Observed(NetworkObservation),
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
                observation_history: NetworkObservationHistory::AwaitingInitial,
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
        match self.reconcile(ReconcileIntent::Observe).await {
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
            state.observation_history = NetworkObservationHistory::Observed(NetworkObservation {
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
        self.reconcile(ReconcileIntent::ForceRepair).await
    }

    async fn reconcile(&self, intent: ReconcileIntent) -> Result<NetworkRoamingSnapshot, Failure> {
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

        let tun_configuration = match &self.gateway {
            Some(gateway) => {
                match crate::runtime_configuration_application::
                    RuntimeConfigurationApplication::from_current_gateway(gateway.as_ref())
                {
                    Ok(configuration) => match configuration.observe_tun().await {
                        Ok(observed) => TunConfigurationObservation::Reported(observed.value),
                        Err(failure) => TunConfigurationObservation::Failed(failure),
                    },
                    Err(failure) => TunConfigurationObservation::Failed(failure),
                }
            }
            None => TunConfigurationObservation::NotComposed,
        };
        let tun_interface = observation
            .interfaces
            .iter()
            .find(|interface| interface.is_up && interface.kind == NetworkInterfaceKind::Tun)
            .map(|interface| interface.name.clone());
        let baseline = match &state.observation_history {
            NetworkObservationHistory::AwaitingInitial => {
                NetworkRoamingBaseline::AwaitingInitialObservation
            }
            NetworkObservationHistory::Observed(previous) => {
                NetworkRoamingBaseline::Observed(previous)
            }
        };
        let decision = decide(baseline, &observation);
        let active_interface = decision.active_interface().map(str::to_owned);
        let gateway_ip = decision.gateway_ip().map(str::to_owned);
        let selected = select_active_interface(&observation.interfaces);
        let (physical_mtu, recommended_tun_mtu, tcp_mss) = selected
            .and_then(|interface| interface.mtu)
            .map(|mtu| {
                let (tun_mtu, tcp_mss) = MtuOptimizer::negotiate_tun_mtu(mtu);
                (Some(mtu), Some(tun_mtu), Some(tcp_mss))
            })
            .unwrap_or_default();
        let mut snapshot = NetworkRoamingSnapshot {
            status: if selected.is_some() {
                NetworkRoamingStatus::Stable
            } else {
                NetworkRoamingStatus::Degraded {
                    reason: decision.reason().to_owned(),
                }
            },
            interfaces: observation.interfaces.clone(),
            active_interface: active_interface.clone(),
            default_gateway: gateway_ip.clone(),
            previous_interface: decision.previous_interface().map(str::to_owned),
            tun_interface: tun_interface.clone(),
            physical_mtu,
            recommended_tun_mtu,
            tcp_mss,
            route_repair_count: state.route_repair_count,
            last_event: state.snapshot.last_event.clone(),
            observed_at_epoch_ms: observation.observed_at_epoch_ms,
            revision,
        };
        match &decision {
            NetworkRoamingDecision::InitialObservation { .. } => {
                snapshot.last_event = Some(NetworkRoamingEvent::InitialObservation {
                    interface: active_interface.clone(),
                    gateway_ip: gateway_ip.clone(),
                });
            }
            NetworkRoamingDecision::InterfaceAddressChanged { interface, .. } => {
                snapshot.last_event = Some(NetworkRoamingEvent::InterfaceAddressChanged {
                    interface: interface.clone(),
                });
            }
            NetworkRoamingDecision::GatewayChanged {
                previous_interface,
                active_interface,
                previous_gateway_ip,
                gateway_ip,
            } => {
                snapshot.last_event = Some(NetworkRoamingEvent::GatewayChanged {
                    old_interface: previous_interface.clone(),
                    new_interface: active_interface.clone(),
                    old_gateway_ip: previous_gateway_ip.clone(),
                    new_gateway_ip: gateway_ip.clone(),
                });
            }
            NetworkRoamingDecision::Stable { .. } => {}
        }

        match plan_route_repair(
            intent,
            &decision,
            selected,
            &tun_configuration,
            tun_interface.as_deref(),
        ) {
            RouteRepairPlan::NotRequested => {}
            RouteRepairPlan::Skipped(reason) => {
                snapshot.last_event = Some(NetworkRoamingEvent::RepairSkipped { reason });
            }
            RouteRepairPlan::Degraded(failure) => {
                snapshot.status = NetworkRoamingStatus::Degraded {
                    reason: failure.message.clone(),
                };
                snapshot.last_event = Some(NetworkRoamingEvent::RepairSkipped {
                    reason: failure.message,
                });
                state.snapshot = snapshot.clone();
                return Ok(snapshot);
            }
            RouteRepairPlan::Failed(failure) => {
                snapshot.status = NetworkRoamingStatus::Failed {
                    failure: failure.clone(),
                };
                snapshot.last_event = Some(NetworkRoamingEvent::RepairFailed {
                    failure: failure.clone(),
                });
                state.snapshot = snapshot;
                return Err(failure);
            }
            RouteRepairPlan::Repair(request) => {
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
            }
        }

        state.observation_history = NetworkObservationHistory::Observed(observation);
        state.snapshot = snapshot.clone();
        Ok(snapshot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    #[cfg(test)]
    use infiltrator_contract::core_identity::{
        CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, CoreVersion, EngineId,
    };
    #[cfg(test)]
    use infiltrator_contract::network_roaming::{
        NetworkInterfaceKind, NetworkInterfaceSnapshot, NetworkObservation,
        NetworkRoamingRepairRequest, NetworkRoamingRepairResult,
    };
    use infiltrator_contract::session::SessionToken;
    use infiltrator_domain::runtime::{MemoryData, ProxyProvider, RuleProvider};
    use infiltrator_ports::error::PortError;
    use infiltrator_ports::runtime_configuration::{
        ConfigurationApply, ConfigurationObservation, ConfigurationRevision, ObservedValue,
        RuntimeConfigurationPorts, TunConfiguration, TunConfigurationPort,
    };
    use infiltrator_ports::runtime_gateway::RuntimeGateway;
    #[cfg(test)]
    use std::sync;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct FakePort {
        observations: sync::Mutex<Vec<NetworkObservation>>,
        repairs: AtomicUsize,
        repair_outcome: FakeRepairOutcome,
    }

    #[derive(Clone, Copy)]
    enum FakeRepairOutcome {
        Repaired,
        Failed,
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
            if matches!(self.repair_outcome, FakeRepairOutcome::Failed) {
                return Err(PortError::Failed("fixture route repair failure".to_owned()));
            }
            Ok(NetworkRoamingRepairResult {
                route_generation: 2,
                detail: "test route readback matched".to_owned(),
            })
        }
    }

    struct FakeGateway {
        scope: CoreScope,
        tun_port: Arc<FakeTunConfiguration>,
    }

    struct FakeTunConfiguration {
        observations: sync::Mutex<Vec<TunConfiguration>>,
    }

    #[async_trait]
    impl TunConfigurationPort for FakeTunConfiguration {
        async fn observe(
            &self,
            scope: &CoreScope,
        ) -> Result<ConfigurationObservation<TunConfiguration>, PortError> {
            let mut observations = self.observations.lock().expect("TUN observations lock");
            let value = if observations.len() > 1 {
                observations.remove(0)
            } else {
                observations
                    .first()
                    .cloned()
                    .expect("TUN configuration fixture")
            };
            Ok(ConfigurationObservation {
                scope: scope.clone(),
                revision: ConfigurationRevision::new(1),
                value,
            })
        }

        async fn apply(
            &self,
            _request: ConfigurationApply<
                infiltrator_ports::runtime_configuration::TunConfigurationUpdate,
            >,
        ) -> Result<ConfigurationObservation<TunConfiguration>, PortError> {
            Err(PortError::unsupported(
                infiltrator_contract::capability::Capability::Tun,
                "roaming fake is read-only",
            ))
        }
    }

    #[async_trait]
    impl RuntimeGateway for FakeGateway {
        fn current_core_scope(&self) -> Result<CoreScope, PortError> {
            Ok(self.scope.clone())
        }

        fn configuration_ports(&self) -> RuntimeConfigurationPorts {
            let mut ports = RuntimeConfigurationPorts::unsupported();
            ports.tun = self.tun_port.clone();
            ports
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

        async fn get_memory(&self) -> Result<MemoryData, PortError> {
            Ok(MemoryData::default())
        }
    }

    fn fake_gateway(auto_route: bool) -> Arc<FakeGateway> {
        fake_gateway_with_configurations(vec![tun_configuration(
            ObservedValue::Reported(true),
            ObservedValue::Reported(auto_route),
            ObservedValue::Reported(true),
        )])
    }

    fn fake_gateway_with_configurations(configurations: Vec<TunConfiguration>) -> Arc<FakeGateway> {
        let scope = CoreScope {
            identity: CoreIdentity::new(
                EngineId::new("mihomo").expect("engine"),
                CoreInstanceId::new("roaming-core").expect("instance"),
                Some(CoreVersion::new("test").expect("version")),
            ),
            generation: CoreGeneration::new(1),
            session_token: Some(SessionToken::new(1)),
        };
        Arc::new(FakeGateway {
            scope,
            tun_port: Arc::new(FakeTunConfiguration {
                observations: sync::Mutex::new(configurations),
            }),
        })
    }

    fn tun_configuration(
        enabled: ObservedValue<bool>,
        auto_route: ObservedValue<bool>,
        strict_route: ObservedValue<bool>,
    ) -> TunConfiguration {
        TunConfiguration {
            enabled,
            stack: ObservedValue::Reported("gvisor".to_owned()),
            auto_route,
            strict_route,
            mtu: ObservedValue::Reported(1420),
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
            observations: sync::Mutex::new(vec![
                observation("wlan0", "192.168.2.1"),
                observation("eth0", "192.168.1.1"),
            ]),
            repairs: AtomicUsize::new(0),
            repair_outcome: FakeRepairOutcome::Repaired,
        });
        let application = NetworkRoamingApplication::new(port.clone(), Some(fake_gateway(true)));
        let first = application.refresh().await;
        assert_eq!(first.status, NetworkRoamingStatus::Stable);
        assert!(matches!(
            first.last_event,
            Some(NetworkRoamingEvent::InitialObservation { .. })
        ));
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
            observations: sync::Mutex::new(vec![
                observation("wlan0", "192.168.2.1"),
                observation("eth0", "192.168.1.1"),
            ]),
            repairs: AtomicUsize::new(0),
            repair_outcome: FakeRepairOutcome::Repaired,
        });
        let application = NetworkRoamingApplication::new(port.clone(), Some(fake_gateway(false)));
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
            observations: sync::Mutex::new(vec![observation("eth0", "192.168.1.1")]),
            repairs: AtomicUsize::new(0),
            repair_outcome: FakeRepairOutcome::Repaired,
        });
        let application = NetworkRoamingApplication::new(port, None);
        let failure = application
            .force_repair()
            .await
            .expect_err("route repair without a gateway must fail closed");
        assert_eq!(failure.code, ErrorCode::Unsupported);
    }

    #[tokio::test]
    async fn failed_route_repair_keeps_the_last_accepted_observation_as_baseline() {
        let port = Arc::new(FakePort {
            observations: sync::Mutex::new(vec![
                observation("wlan0", "192.168.2.1"),
                observation("eth0", "192.168.1.1"),
                observation("eth0", "192.168.1.1"),
            ]),
            repairs: AtomicUsize::new(0),
            repair_outcome: FakeRepairOutcome::Failed,
        });
        let application = NetworkRoamingApplication::new(port.clone(), Some(fake_gateway(true)));

        let initial = application.refresh().await;
        assert!(matches!(
            initial.last_event,
            Some(NetworkRoamingEvent::InitialObservation { .. })
        ));
        let failed_migration = application.refresh().await;
        assert!(matches!(
            failed_migration.last_event,
            Some(NetworkRoamingEvent::RepairFailed { .. })
        ));
        let retried_migration = application.refresh().await;

        assert_eq!(port.repairs.load(Ordering::SeqCst), 2);
        assert!(matches!(
            retried_migration.last_event,
            Some(NetworkRoamingEvent::RepairFailed { .. })
        ));
    }

    #[tokio::test]
    async fn unreported_tun_state_keeps_migration_pending_until_route_repair_is_possible() {
        let port = Arc::new(FakePort {
            observations: sync::Mutex::new(vec![
                observation("wlan0", "192.168.2.1"),
                observation("eth0", "192.168.1.1"),
                observation("eth0", "192.168.1.1"),
            ]),
            repairs: AtomicUsize::new(0),
            repair_outcome: FakeRepairOutcome::Repaired,
        });
        let application = NetworkRoamingApplication::new(
            port.clone(),
            Some(fake_gateway_with_configurations(vec![
                tun_configuration(
                    ObservedValue::Reported(true),
                    ObservedValue::Reported(true),
                    ObservedValue::Reported(true),
                ),
                tun_configuration(
                    ObservedValue::Unreported,
                    ObservedValue::Reported(true),
                    ObservedValue::Reported(true),
                ),
                tun_configuration(
                    ObservedValue::Reported(true),
                    ObservedValue::Reported(true),
                    ObservedValue::Reported(true),
                ),
            ])),
        );

        let _ = application.refresh().await;
        let unavailable = application.refresh().await;
        assert!(matches!(
            unavailable.status,
            NetworkRoamingStatus::Degraded { .. }
        ));
        assert!(matches!(
            unavailable.last_event,
            Some(NetworkRoamingEvent::RepairSkipped { .. })
        ));
        assert_eq!(port.repairs.load(Ordering::SeqCst), 0);

        let retried = application.refresh().await;
        assert!(matches!(
            retried.last_event,
            Some(NetworkRoamingEvent::RoutesRepaired { .. })
        ));
        assert_eq!(port.repairs.load(Ordering::SeqCst), 1);
    }
}
