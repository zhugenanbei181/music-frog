//! Runtime-neutral physical-link roaming and TUN route recovery use-case.

use crate::dns_cache_application::DnsCacheApplication;
use crate::runtime_configuration_application::RuntimeConfigurationApplication;
use crate::runtime_scope::active_scope;
use futures_util::lock::Mutex;
use infiltrator_contract::network_roaming::{
    NetworkInterfaceKind, NetworkObservation, NetworkRoamingEvent, NetworkRoamingSnapshot,
    NetworkRoamingStatus,
};
use infiltrator_domain::mtu_optimizer::MtuOptimizer;
use infiltrator_domain::network_roaming::{
    NetworkRoamingBaseline, NetworkRoamingDecision, decide, select_active_interface,
};
use infiltrator_ports::network_roaming::NetworkRoamingPort;
use infiltrator_ports::runtime_configuration::RuntimeConfigurationPorts;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_ports::core_scope::CoreScopeSource;
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
    network_generation: u64,
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
    core_configuration: CoreConfigurationOwner,
    dns_cache: Option<DnsCacheApplication>,
    state: Arc<Mutex<State>>,
}

#[derive(Clone)]
enum CoreConfigurationOwner {
    NotComposed,
    Composed {
        scope_source: Arc<dyn CoreScopeSource>,
        ports: RuntimeConfigurationPorts,
    },
}

impl NetworkRoamingApplication {
    pub fn new(port: Arc<dyn NetworkRoamingPort>) -> Self {
        Self {
            port,
            core_configuration: CoreConfigurationOwner::NotComposed,
            dns_cache: None,
            state: Arc::new(Mutex::new(State {
                observation_history: NetworkObservationHistory::AwaitingInitial,
                snapshot: NetworkRoamingSnapshot::default(),
                next_revision: 1,
                route_repair_count: 0,
                network_generation: 0,
            })),
        }
    }

    pub fn with_dns_cache(mut self, dns_cache: DnsCacheApplication) -> Self {
        self.dns_cache = Some(dns_cache);
        self
    }

    pub fn with_core_configuration(
        mut self,
        scope_source: Arc<dyn CoreScopeSource>,
        ports: RuntimeConfigurationPorts,
    ) -> Self {
        self.core_configuration = CoreConfigurationOwner::Composed {
            scope_source,
            ports,
        };
        self
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

        let tun_configuration = match &self.core_configuration {
            CoreConfigurationOwner::NotComposed => TunConfigurationObservation::NotComposed,
            CoreConfigurationOwner::Composed {
                scope_source,
                ports,
            } => match active_scope(scope_source.as_ref()) {
                Ok(scope) => match RuntimeConfigurationApplication::new(scope, ports.clone())
                    .observe_tun()
                    .await
                {
                    Ok(observed) => TunConfigurationObservation::Reported(observed.value),
                    Err(failure) => TunConfigurationObservation::Failed(failure),
                },
                Err(failure) => TunConfigurationObservation::Failed(failure),
            },
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
            network_generation: state.network_generation,
        };
        let mut physical_network_changed = false;
        match &decision {
            NetworkRoamingDecision::InitialObservation { .. } => {
                snapshot.last_event = Some(NetworkRoamingEvent::InitialObservation {
                    interface: active_interface.clone(),
                    gateway_ip: gateway_ip.clone(),
                });
            }
            NetworkRoamingDecision::InterfaceAddressChanged { interface, .. } => {
                physical_network_changed = true;
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
                physical_network_changed = true;
                snapshot.last_event = Some(NetworkRoamingEvent::GatewayChanged {
                    old_interface: previous_interface.clone(),
                    new_interface: active_interface.clone(),
                    old_gateway_ip: previous_gateway_ip.clone(),
                    new_gateway_ip: gateway_ip.clone(),
                });
            }
            NetworkRoamingDecision::Stable { .. } => {}
        }

        if physical_network_changed {
            state.network_generation = state.network_generation.saturating_add(1);
        }
        snapshot.network_generation = state.network_generation;

        if physical_network_changed && let Some(dns_cache) = self.dns_cache.as_ref() {
            if let CoreConfigurationOwner::Composed { scope_source, .. } = &self.core_configuration
            {
                if let Ok(scope) = active_scope(scope_source.as_ref()) {
                    let _ = dns_cache
                        .auto_flush_for_network_change(scope, state.network_generation)
                        .await;
                }
            }
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
    use infiltrator_contract::network_roaming::{
        NetworkInterfaceKind, NetworkInterfaceSnapshot, NetworkObservation,
        NetworkRoamingRepairRequest, NetworkRoamingRepairResult,
    };
    use infiltrator_ports::runtime_configuration::TunConfigurationUpdate;
    use infiltrator_ports::runtime_configuration::{
        ConfigurationApply, ConfigurationObservation, ConfigurationRevision, ObservedValue,
        RuntimeConfigurationPorts, TunConfiguration, TunConfigurationPort,
    };
    use lurker_core_contract::capability::Capability;
    #[cfg(test)]
    use lurker_core_contract::identity::{
        CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, CoreVersion, EngineId,
    };
    use lurker_core_contract::session::SessionToken;
    use lurker_core_ports::core_scope::{CoreScopeObservation, CoreScopeSource};
    use lurker_core_ports::error::PortError;
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

    struct FakeCoreConfigurationOwner {
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
            _request: ConfigurationApply<TunConfigurationUpdate>,
        ) -> Result<ConfigurationObservation<TunConfiguration>, PortError> {
            Err(PortError::unsupported(
                Capability::Tun,
                "roaming fake is read-only",
            ))
        }
    }

    impl CoreScopeSource for FakeCoreConfigurationOwner {
        fn scope_observation(&self) -> CoreScopeObservation {
            CoreScopeObservation::Observed(self.scope.clone())
        }
    }

    impl FakeCoreConfigurationOwner {
        fn configuration_ports(&self) -> RuntimeConfigurationPorts {
            let mut ports = RuntimeConfigurationPorts::unsupported();
            ports.tun = self.tun_port.clone();
            ports
        }
    }

    fn fake_core_configuration_owner(auto_route: bool) -> Arc<FakeCoreConfigurationOwner> {
        fake_core_configuration_with_configurations(vec![tun_configuration(
            ObservedValue::Reported(true),
            ObservedValue::Reported(auto_route),
            ObservedValue::Reported(true),
        )])
    }

    fn fake_core_configuration_with_configurations(
        configurations: Vec<TunConfiguration>,
    ) -> Arc<FakeCoreConfigurationOwner> {
        let scope = CoreScope {
            identity: CoreIdentity::new(
                EngineId::new("mihomo").expect("engine"),
                CoreInstanceId::new("roaming-core").expect("instance"),
                Some(CoreVersion::new("test").expect("version")),
            ),
            generation: CoreGeneration::new(1),
            session_token: Some(SessionToken::new(1)),
        };
        Arc::new(FakeCoreConfigurationOwner {
            scope,
            tun_port: Arc::new(FakeTunConfiguration {
                observations: sync::Mutex::new(configurations),
            }),
        })
    }

    fn with_fake_core_configuration(
        application: NetworkRoamingApplication,
        owner: Arc<FakeCoreConfigurationOwner>,
    ) -> NetworkRoamingApplication {
        let scope_source: Arc<dyn CoreScopeSource> = owner.clone();
        application.with_core_configuration(scope_source, owner.configuration_ports())
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
        let application = with_fake_core_configuration(
            NetworkRoamingApplication::new(port.clone()),
            fake_core_configuration_owner(true),
        );
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
        let application = with_fake_core_configuration(
            NetworkRoamingApplication::new(port.clone()),
            fake_core_configuration_owner(false),
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
            observations: sync::Mutex::new(vec![observation("eth0", "192.168.1.1")]),
            repairs: AtomicUsize::new(0),
            repair_outcome: FakeRepairOutcome::Repaired,
        });
        let application = NetworkRoamingApplication::new(port);
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
        let application = with_fake_core_configuration(
            NetworkRoamingApplication::new(port.clone()),
            fake_core_configuration_owner(true),
        );

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
        let application = with_fake_core_configuration(
            NetworkRoamingApplication::new(port.clone()),
            fake_core_configuration_with_configurations(vec![
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
            ]),
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

    #[tokio::test]
    async fn migration_automatically_flushes_dns_cache_with_new_generation_once() {
        use infiltrator_test_support::dns_cache_fixtures::IsolatedCaches;

        let port = Arc::new(FakePort {
            observations: sync::Mutex::new(vec![
                observation("wlan0", "192.168.2.1"),
                observation("eth0", "192.168.1.1"),
                observation("eth0", "192.168.1.1"),
            ]),
            repairs: AtomicUsize::new(0),
            repair_outcome: FakeRepairOutcome::Repaired,
        });

        let caches = Arc::new(IsolatedCaches::default());
        let dns_cache = DnsCacheApplication::new(Some(caches.clone()), Some(caches.clone()));

        let application = with_fake_core_configuration(
            NetworkRoamingApplication::new(port.clone()),
            fake_core_configuration_owner(true),
        )
        .with_dns_cache(dns_cache.clone());

        // 1. Initial observation
        let first = application.refresh().await;
        assert_eq!(first.network_generation, 0);
        assert_eq!(caches.fake_calls.load(Ordering::SeqCst), 0);
        assert_eq!(caches.system_calls.load(Ordering::SeqCst), 0);

        // 2. Network change (wlan0 -> eth0)
        let second = application.refresh().await;
        assert_eq!(second.network_generation, 1);
        assert_eq!(caches.fake_calls.load(Ordering::SeqCst), 1);
        assert_eq!(caches.system_calls.load(Ordering::SeqCst), 1);
        let dns_snapshot = dns_cache.snapshot();
        assert_eq!(dns_snapshot.auto_flush_generation, Some(1));
        assert!(dns_snapshot.report.fake_ip.is_flushed());
        assert!(dns_snapshot.report.os_cache.is_flushed());

        // 3. Stable observation (eth0 -> eth0)
        let third = application.refresh().await;
        assert_eq!(third.network_generation, 1);
        assert_eq!(caches.fake_calls.load(Ordering::SeqCst), 1);
        assert_eq!(caches.system_calls.load(Ordering::SeqCst), 1);
    }
}
