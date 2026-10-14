//! Behavior tests for the product surface reader.

use super::*;
use async_trait::async_trait;
use infiltrator_contract::android_control::{
    VpnAuthorizationState, VpnOwnerCoreReadback, VpnOwnerLifecycle, VpnOwnerReadiness,
    VpnOwnerResourceFact, VpnOwnerResourceKind, VpnOwnerResourceState, VpnOwnerSnapshot,
};
use infiltrator_contract::command::{CommandIntent, CommandResult};
use infiltrator_contract::controller::ControllerAuthStatus;
use infiltrator_contract::core_capabilities::{
    AdapterCapabilityDecision, AdapterRevision, CoreCapabilityEntry, CoreCapabilityMatrix,
    CoreCapabilityScope, CoreLifecycleOperation, CoreOperation, EngineCapabilityDecision,
    HostCapabilityDecision, OutboundOperation,
};
use infiltrator_contract::core_identity::{
    CoreIdentity, CoreInstanceId, CoreScope, CoreVersion, EngineId,
};
use infiltrator_contract::error::ErrorCode;
use infiltrator_contract::mtu::PhysicalMtuSnapshot;
use infiltrator_contract::network_roaming::{
    NetworkInterfaceKind, NetworkInterfaceSnapshot, NetworkObservation,
    NetworkRoamingRepairRequest, NetworkRoamingRepairResult,
};
use infiltrator_contract::offline_startup::{LocalAssetStatus, OfflineStartupSnapshot};
use infiltrator_contract::outbound::inventory::OutboundSelectionState;
use infiltrator_contract::outbound::inventory::SelectionAvailability;
use infiltrator_contract::outbound::{
    details::{
        OutboundDetail, OutboundDetailsSnapshot, OutboundDetailsState, OutboundHealthObservation,
        OutboundObservation, OutboundProbeAvailability, OutboundProtocolObservation,
        OutboundSnapshot, OutboundUdpSupport,
    },
    identity::{OutboundId, SnapshotRevision},
    inventory::{
        OutboundInventorySnapshot, OutboundInventoryState, OutboundProbeSnapshot,
        OutboundSelectionSet,
    },
};
use infiltrator_contract::port_conflict::{PortBinding, PortConflict, PortConflictSnapshot};
use infiltrator_contract::privileged_network::PrivilegedNetworkState;
use infiltrator_contract::runtime_status::{
    ControlServiceObservation, CoreProcessObservation, ProxyEffectivenessObservation,
    ProxyReadinessBlocker, TunDataPlaneObservation, VpnAuthorizationObservation,
};
use infiltrator_contract::service_mode::{
    ServiceModePlatform, ServiceModeSnapshot, ServiceModeState,
};
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_contract::surface_snapshot::PageStatus;
use infiltrator_contract::system_proxy::SystemProxyObservation;
use infiltrator_contract::version::{
    CoreChannelStatus, CoreRelease, CoreReleaseSummary, CoreRollbackSnapshot, EngineReleaseChannel,
    InstalledCoreVersion,
};
use infiltrator_contract::vpn::{
    PreparedVpnConfiguration, VpnGeneration, VpnSessionSnapshot, VpnSessionState, VpnStartRequest,
    VpnSurfaceObservation,
};
use infiltrator_ports::application_runtime::{
    ApplicationFuture, ApplicationRuntime, ApplicationSleep,
};
use infiltrator_ports::core_capability_provider::{
    CoreCapabilityProvider, CoreCapabilityProviderError,
};
use infiltrator_ports::core_process::CoreProcessStartContext;
use infiltrator_ports::core_process::{CoreProcess, CoreReadiness};
use infiltrator_ports::endpoint::{ControllerEndpoint, EndpointSource};
use infiltrator_ports::mtu_probe::MtuProbePort;
use infiltrator_ports::network_roaming::NetworkRoamingPort;
use infiltrator_ports::offline_startup::OfflineStartupPort;
use infiltrator_ports::outbound::{OutboundPort, OutboundProbeRequest};
use infiltrator_ports::port_conflict::PortConflictPort;
use infiltrator_ports::service_mode::ServiceModePort;
use infiltrator_ports::system_proxy::SystemProxyPort;
use infiltrator_ports::version::{
    VersionActivationRequest, VersionInstallRequest, VersionPort, VersionUninstallRequest,
};
use infiltrator_ports::vpn_service::{VpnServicePort, VpnStartReceipt};
use infiltrator_test_support::doctor_capture_fixtures::DoctorCapturePort;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;
use tokio::runtime::Builder;
use tokio::time::sleep;

struct TestRuntime;

impl ApplicationRuntime for TestRuntime {
    fn block_on(&self, future: ApplicationFuture) {
        Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime")
            .block_on(future);
    }

    fn sleep(&self, duration: Duration) -> ApplicationSleep<'_> {
        Box::pin(sleep(duration))
    }
}

#[tokio::test]
async fn product_surface_projects_typed_doctor_observation_and_retains_report_on_failure() {
    let doctor = DoctorApplication::new(Arc::new(DoctorCapturePort::default()));
    let core = Arc::new(CoreApplication::new(
        Arc::new(TestProcess),
        Arc::new(TestReadiness),
        Arc::new(TestRuntime),
    ));
    let reader = ProductSurfaceReader::new(core, SurfaceKind::BevyDesktop, HostKind::Desktop)
        .with_doctor(doctor.clone());

    let empty = reader.read().await.expect("unobserved surface read");
    assert_eq!(empty.pages.doctor.status, PageStatus::Empty);

    doctor.run(None).await.expect("first diagnostic report");
    let ready = reader.read().await.expect("ready surface read");
    assert_eq!(ready.pages.doctor.status, PageStatus::Ready);
    let report = ready
        .pages
        .doctor
        .data
        .as_ref()
        .expect("ready doctor page")
        .clone();
    assert_eq!(report.report_finished_at, Some(20));

    let failure = doctor.run(None).await.expect_err("second run is denied");
    assert_eq!(failure.code, ErrorCode::Permission);
    let failed = reader.read().await.expect("failed surface read");
    assert_eq!(failed.pages.doctor.status, PageStatus::Failed { failure });
    assert_eq!(failed.pages.doctor.data, Some(report));
}

struct TestProcess;

#[async_trait]
impl CoreProcess for TestProcess {
    async fn start(&self, _context: CoreProcessStartContext) -> Result<(), PortError> {
        Ok(())
    }

    async fn stop(&self) -> Result<(), PortError> {
        Ok(())
    }

    async fn status(&self) -> Result<CoreLifecycle, PortError> {
        Ok(CoreLifecycle::Stopped)
    }

    fn controller_endpoint(&self) -> Option<String> {
        Some("http://127.0.0.1:9090".to_owned())
    }
}

struct ScopeProcess {
    running: AtomicBool,
}

#[async_trait]
impl CoreProcess for ScopeProcess {
    fn core_identity(&self) -> Option<CoreIdentity> {
        Some(CoreIdentity::new(
            EngineId::new("mihomo").expect("engine ID"),
            CoreInstanceId::new("surface-reader-test").expect("instance ID"),
            None,
        ))
    }

    async fn start(&self, _context: CoreProcessStartContext) -> Result<(), PortError> {
        self.running.store(true, Ordering::SeqCst);
        Ok(())
    }

    async fn stop(&self) -> Result<(), PortError> {
        self.running.store(false, Ordering::SeqCst);
        Ok(())
    }

    async fn status(&self) -> Result<CoreLifecycle, PortError> {
        if self.running.load(Ordering::SeqCst) {
            Ok(CoreLifecycle::Running)
        } else {
            Ok(CoreLifecycle::Stopped)
        }
    }

    fn controller_endpoint(&self) -> Option<String> {
        Some("http://127.0.0.1:9090".to_owned())
    }
}

struct TestReadiness;

#[async_trait]
impl CoreReadiness for TestReadiness {
    async fn probe(&self) -> Result<String, PortError> {
        Ok("http://127.0.0.1:9090".to_owned())
    }
}

struct OutboundReadiness;

#[async_trait]
impl CoreReadiness for OutboundReadiness {
    async fn probe(&self) -> Result<String, PortError> {
        Ok("http://127.0.0.1:9090".to_owned())
    }

    async fn active_core_version(&self) -> Result<Option<CoreVersion>, PortError> {
        Ok(Some(
            CoreVersion::new("v1.19.32").expect("valid test version"),
        ))
    }
}

struct TestEndpoint;

#[async_trait]
impl EndpointSource for TestEndpoint {
    async fn resolve(&self) -> Result<ControllerEndpoint, PortError> {
        Ok(ControllerEndpoint {
            url: "http://127.0.0.1:9090".to_owned(),
            secret: Some("generated-by-host".to_owned()),
        })
    }
}

struct TestServiceMode;

#[async_trait]
impl ServiceModePort for TestServiceMode {
    async fn snapshot(&self) -> Result<ServiceModeSnapshot, PortError> {
        Ok(ServiceModeSnapshot {
            platform: ServiceModePlatform::LinuxPolkit,
            state: ServiceModeState::Ready,
        })
    }

    async fn prepare(&self) -> Result<ServiceModeSnapshot, PortError> {
        self.snapshot().await
    }
}

struct TestPortConflicts;

#[async_trait]
impl PortConflictPort for TestPortConflicts {
    async fn snapshot(&self) -> Result<PortConflictSnapshot, PortError> {
        Ok(PortConflictSnapshot {
            revision: 1,
            conflicts: vec![PortConflict {
                binding: PortBinding::Controller,
                port: 9090,
                available: false,
                owner_pid: Some(4242),
                owner_name: Some("mihomo".to_owned()),
                can_release: true,
            }],
        })
    }

    async fn repair(&self) -> Result<PortConflictSnapshot, PortError> {
        Ok(PortConflictSnapshot::default())
    }
}

struct TestOfflineStartup;

#[async_trait]
impl OfflineStartupPort for TestOfflineStartup {
    async fn validate_offline_startup(&self) -> Result<OfflineStartupSnapshot, PortError> {
        Ok(OfflineStartupSnapshot::ready(LocalAssetStatus::Available))
    }
}

struct TestMtu {
    calls: Arc<AtomicUsize>,
}

#[async_trait]
impl MtuProbePort for TestMtu {
    async fn probe_physical_mtu(&self) -> Result<PhysicalMtuSnapshot, PortError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(PhysicalMtuSnapshot {
            interface: "eth0".to_owned(),
            mtu: 1500,
        })
    }
}

struct TestSystemProxy {
    calls: Arc<AtomicUsize>,
}

struct StopCoreDuringSurfaceRead {
    core: Arc<CoreApplication>,
}

struct TestNetworkRoaming {
    calls: Arc<AtomicUsize>,
}

struct TestVpn;

#[async_trait]
impl VpnServicePort for TestVpn {
    async fn authorization_state(&self) -> Result<VpnAuthorizationState, PortError> {
        Ok(VpnAuthorizationState::Granted)
    }

    async fn owner_observation(&self) -> VpnSurfaceObservation {
        VpnSurfaceObservation::from_android_owner(VpnOwnerSnapshot {
            owner_session_id: "test-vpn-owner".to_owned(),
            revision: 4,
            vpn_generation: VpnGeneration::new(3),
            vpn_state: VpnOwnerLifecycle::Running,
            readiness: VpnOwnerReadiness::Ready,
            resources: [
                VpnOwnerResourceKind::PreparedConfiguration,
                VpnOwnerResourceKind::Worker,
                VpnOwnerResourceKind::TunnelDescriptor,
                VpnOwnerResourceKind::RoutingLease,
            ]
            .into_iter()
            .map(|kind| VpnOwnerResourceFact {
                kind,
                state: match kind {
                    VpnOwnerResourceKind::Worker | VpnOwnerResourceKind::TunnelDescriptor => {
                        VpnOwnerResourceState::Held { generation: 3 }
                    }
                    VpnOwnerResourceKind::PreparedConfiguration
                    | VpnOwnerResourceKind::RoutingLease => VpnOwnerResourceState::Absent,
                },
            })
            .collect(),
            foreground: true,
            mtu: Some(1500),
            route_count: 2,
            dns_servers: vec!["1.1.1.1".to_owned()],
            ipv6: true,
            core: VpnOwnerCoreReadback::Unavailable {
                failure_message: "test Core is not scoped".to_owned(),
            },
        })
    }

    async fn request_start(&self) -> Result<VpnSessionSnapshot, PortError> {
        Ok(VpnSessionSnapshot {
            state: VpnSessionState::Starting,
            foreground: true,
            ..Default::default()
        })
    }

    async fn prepare(
        &self,
        _prepared: PreparedVpnConfiguration,
    ) -> Result<VpnSessionSnapshot, PortError> {
        Ok(VpnSessionSnapshot {
            state: VpnSessionState::Starting,
            foreground: true,
            ..Default::default()
        })
    }

    async fn start(&self, request: VpnStartRequest) -> Result<VpnStartReceipt, PortError> {
        let configuration = &request.prepared.configuration;
        Ok(VpnStartReceipt::Started(VpnSessionSnapshot::running(
            1,
            configuration.mtu,
            configuration.routes.len(),
            configuration.dns_servers.clone(),
            configuration.ipv6,
            true,
        )))
    }

    async fn stop(&self) -> Result<VpnSessionSnapshot, PortError> {
        Ok(VpnSessionSnapshot {
            state: VpnSessionState::Stopped,
            ..Default::default()
        })
    }

    async fn revoke(&self) -> Result<VpnSessionSnapshot, PortError> {
        Ok(VpnSessionSnapshot {
            state: VpnSessionState::Revoked,
            ..Default::default()
        })
    }

    async fn snapshot(&self) -> Result<VpnSessionSnapshot, PortError> {
        Ok(VpnSessionSnapshot::running(
            1,
            1500,
            2,
            vec!["1.1.1.1".to_owned()],
            true,
            true,
        ))
    }
}

#[async_trait]
impl NetworkRoamingPort for TestNetworkRoaming {
    async fn observe(&self) -> Result<NetworkObservation, PortError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(NetworkObservation {
            interfaces: vec![NetworkInterfaceSnapshot {
                name: "eth0".to_owned(),
                kind: NetworkInterfaceKind::Ethernet,
                is_up: true,
                is_default_gateway: true,
                gateway_ip: Some("192.0.2.1".to_owned()),
                ip_addresses: vec!["192.0.2.10/24".to_owned()],
                mtu: Some(1500),
                metric: Some(100),
                dns_servers: Vec::new(),
            }],
            observed_at_epoch_ms: Some(1),
        })
    }

    async fn repair(
        &self,
        _request: NetworkRoamingRepairRequest,
    ) -> Result<NetworkRoamingRepairResult, PortError> {
        Ok(NetworkRoamingRepairResult {
            route_generation: 1,
            detail: "test readback".to_owned(),
        })
    }
}

#[async_trait]
impl SystemProxyPort for TestSystemProxy {
    async fn snapshot(&self) -> Result<SystemProxyObservation, PortError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(SystemProxyObservation {
            enabled: true,
            endpoint: Some("127.0.0.1:7890".to_owned()),
            bypass: Some("localhost".to_owned()),
        })
    }

    async fn apply(
        &self,
        _endpoint: Option<String>,
        _bypass: Option<String>,
    ) -> Result<(), PortError> {
        Ok(())
    }
}

#[async_trait]
impl SystemProxyPort for StopCoreDuringSurfaceRead {
    async fn snapshot(&self) -> Result<SystemProxyObservation, PortError> {
        if !matches!(
            test_execute_result_at_current_scope(&self.core, CommandIntent::StopCore).await,
            CommandResult::Completed { .. }
        ) {
            return Err(PortError::Failed(
                "test core stop did not complete".to_owned(),
            ));
        }
        Ok(SystemProxyObservation {
            enabled: false,
            endpoint: None,
            bypass: None,
        })
    }

    async fn apply(
        &self,
        _endpoint: Option<String>,
        _bypass: Option<String>,
    ) -> Result<(), PortError> {
        Ok(())
    }
}

struct TestVersions {
    engine_id: EngineId,
    calls: Arc<AtomicUsize>,
}

#[async_trait]
impl VersionPort for TestVersions {
    fn engine_id(&self) -> EngineId {
        self.engine_id.clone()
    }

    async fn list_installed(
        &self,
        _engine_id: &EngineId,
    ) -> Result<Vec<InstalledCoreVersion>, PortError> {
        Ok(Vec::new())
    }

    async fn latest(&self, channel: EngineReleaseChannel) -> Result<CoreRelease, PortError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(CoreRelease {
            engine_id: VersionPort::engine_id(self),
            version: format!("{}-v1.0.0", channel.channel_id.as_str()),
            release_date: "2026-09-05".to_owned(),
        })
    }

    async fn list_releases(
        &self,
        _engine_id: &EngineId,
        _limit: usize,
    ) -> Result<Vec<CoreReleaseSummary>, PortError> {
        Ok(Vec::new())
    }

    async fn install(&self, _request: VersionInstallRequest) -> Result<(), PortError> {
        Ok(())
    }

    async fn activate(&self, _request: VersionActivationRequest) -> Result<(), PortError> {
        Ok(())
    }

    async fn uninstall(&self, _request: VersionUninstallRequest) -> Result<(), PortError> {
        Ok(())
    }

    async fn rollback_snapshot(
        &self,
        engine_id: &EngineId,
    ) -> Result<CoreRollbackSnapshot, PortError> {
        Ok(CoreRollbackSnapshot {
            engine_id: engine_id.clone(),
            current: Some("v1.19.30".to_owned()),
            target: Some("v1.19.29".to_owned()),
            history: vec!["v1.19.29".to_owned()],
        })
    }
}

#[tokio::test]
async fn surface_reader_publishes_and_caches_all_core_channel_results() {
    let calls = Arc::new(AtomicUsize::new(0));
    let mtu_calls = Arc::new(AtomicUsize::new(0));
    let proxy_calls = Arc::new(AtomicUsize::new(0));
    let network_calls = Arc::new(AtomicUsize::new(0));
    let core = Arc::new(CoreApplication::new(
        Arc::new(TestProcess),
        Arc::new(TestReadiness),
        Arc::new(TestRuntime),
    ));
    let reader = ProductSurfaceReader::new(core, SurfaceKind::BevyDesktop, HostKind::Desktop)
        .with_versions(
            VersionApplication::new(
                EngineId::new("mihomo").unwrap(),
                Arc::new(TestVersions {
                    engine_id: EngineId::new("mihomo").unwrap(),
                    calls: calls.clone(),
                }),
            )
            .unwrap(),
        )
        .with_endpoint_source(Arc::new(TestEndpoint))
        .with_service_mode(ServiceModeApplication::new(Arc::new(TestServiceMode)))
        .with_port_conflicts(PortConflictApplication::new(Arc::new(TestPortConflicts)))
        .with_offline_startup(OfflineStartupApplication::new(Arc::new(TestOfflineStartup)))
        .with_mtu(MtuApplication::new(Arc::new(TestMtu {
            calls: mtu_calls.clone(),
        })))
        .with_system_proxy(SystemProxyApplication::new(Arc::new(TestSystemProxy {
            calls: proxy_calls.clone(),
        })))
        .with_network_roaming(NetworkRoamingApplication::new(Arc::new(
            TestNetworkRoaming {
                calls: network_calls.clone(),
            },
        )))
        .with_vpn(VpnServiceApplication::new(Arc::new(TestVpn)));

    let first = reader.read().await.expect("first surface read");
    let second = reader.read().await.expect("cached surface read");
    assert!(
        second.revision > first.revision,
        "non-core observations remain refreshable"
    );
    assert_eq!(first.versions.channels.len(), 2);
    assert_eq!(first.versions.revision, 1);
    assert_eq!(second.versions.revision, 1);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(
        first
            .versions
            .channels
            .iter()
            .map(|channel| channel.channel.channel_id.as_str())
            .collect::<Vec<_>>(),
        vec!["stable", "alpha"]
    );
    assert_eq!(first.versions.rollback.target.as_deref(), Some("v1.19.29"));
    assert_eq!(first.controller_auth.status, ControllerAuthStatus::Secured);
    assert_eq!(first.service_mode.state, ServiceModeState::Ready);
    assert!(first.port_conflicts.has_conflicts());
    assert_eq!(first.port_conflicts.conflicts[0].owner_pid, Some(4242));
    assert!(first.offline_startup.is_offline_startable());
    assert_eq!(first.offline_startup.geoip, LocalAssetStatus::Available);
    assert!(first.mtu.is_ready());
    assert_eq!(first.mtu.physical_interface.as_deref(), Some("eth0"));
    assert_eq!(first.mtu.tun_mtu, Some(1420));
    assert_eq!(first.mtu.applied_tun_mtu, None);
    assert_eq!(mtu_calls.load(Ordering::SeqCst), 1);
    assert!(first.system_proxy.is_enabled());
    assert_eq!(
        first.system_proxy.endpoint.as_deref(),
        Some("127.0.0.1:7890")
    );
    assert_eq!(proxy_calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        first.network_roaming.active_interface.as_deref(),
        Some("eth0")
    );
    assert_eq!(network_calls.load(Ordering::SeqCst), 2);
    assert!(matches!(
        &first.vpn,
        VpnSurfaceObservation::AndroidOwner(owner)
            if owner.is_running() && owner.route_count == 2
    ));
    assert!(matches!(
        first.privileged_network.state,
        PrivilegedNetworkState::Unsupported { .. }
    ));
    assert!(
        first
            .versions
            .channels
            .iter()
            .all(|channel| matches!(channel.status, CoreChannelStatus::Ready { .. }))
    );
}

#[tokio::test]
async fn surface_reader_publishes_android_authorization_separately_from_runtime_readiness() {
    let core = Arc::new(CoreApplication::new(
        Arc::new(TestProcess),
        Arc::new(TestReadiness),
        Arc::new(TestRuntime),
    ));
    let reader = ProductSurfaceReader::new(core, SurfaceKind::AndroidCompose, HostKind::Android)
        .with_vpn(VpnServiceApplication::new(Arc::new(TestVpn)));

    let snapshot = reader.read().await.expect("Android surface read");

    assert_eq!(
        snapshot.runtime_status.vpn_authorization,
        VpnAuthorizationObservation::Granted
    );
    assert!(matches!(
        snapshot.runtime_status.control_service,
        ControlServiceObservation::Available { .. }
    ));
    assert_eq!(
        snapshot.runtime_status.core_process,
        CoreProcessObservation::Unselected
    );
    assert!(matches!(
        snapshot.runtime_status.tun_data_plane,
        TunDataPlaneObservation::Inconsistent { .. }
    ));
    assert_eq!(
        snapshot.runtime_status.proxy_effectiveness,
        ProxyEffectivenessObservation::NotReady {
            blocker: ProxyReadinessBlocker::CoreNotRunning,
        }
    );
}

#[tokio::test]
async fn cloned_surface_readers_do_not_reuse_another_engines_version_cache() {
    let core = Arc::new(CoreApplication::new(
        Arc::new(TestProcess),
        Arc::new(TestReadiness),
        Arc::new(TestRuntime),
    ));
    let base = ProductSurfaceReader::new(core, SurfaceKind::BevyDesktop, HostKind::Desktop);
    let mihomo_id = EngineId::new("mihomo").unwrap();
    let mihomo = VersionApplication::new(
        mihomo_id.clone(),
        Arc::new(TestVersions {
            engine_id: mihomo_id.clone(),
            calls: Arc::new(AtomicUsize::new(0)),
        }),
    )
    .unwrap();
    let mihomo_snapshot = base
        .clone()
        .with_versions(mihomo)
        .read()
        .await
        .expect("Mihomo surface read");
    let sing_box_id = EngineId::new("sing-box").unwrap();
    let sing_box = VersionApplication::new(
        sing_box_id.clone(),
        Arc::new(TestVersions {
            engine_id: sing_box_id.clone(),
            calls: Arc::new(AtomicUsize::new(0)),
        }),
    )
    .unwrap();

    let sing_box_snapshot = base
        .with_versions(sing_box)
        .read()
        .await
        .expect("sing-box surface read");

    assert_eq!(mihomo_snapshot.versions.engine_id, mihomo_id);
    assert_eq!(sing_box_snapshot.versions.engine_id, sing_box_id);
    assert_eq!(sing_box_snapshot.versions.revision, 1);
    assert!(
        sing_box_snapshot
            .versions
            .channels
            .iter()
            .all(|channel| channel.engine_id == sing_box_id)
    );
}

#[tokio::test]
async fn surface_reader_rejects_a_snapshot_when_core_scope_changes_mid_read() {
    let core = Arc::new(CoreApplication::new(
        Arc::new(ScopeProcess {
            running: AtomicBool::new(false),
        }),
        Arc::new(TestReadiness),
        Arc::new(TestRuntime),
    ));
    install_lifecycle_capabilities(&core);
    assert!(matches!(
        test_execute_result_at_current_scope(&core, CommandIntent::StartCore).await,
        CommandResult::Completed { .. }
    ));
    let starting_scope = core.snapshot().scope().expect("running core scope");
    let reader =
        ProductSurfaceReader::new(core.clone(), SurfaceKind::BevyDesktop, HostKind::Desktop)
            .with_system_proxy(SystemProxyApplication::new(Arc::new(
                StopCoreDuringSurfaceRead { core: core.clone() },
            )));

    let result = reader.read().await;

    assert!(matches!(
        result,
        Err(PortError::Rejected(failure))
            if failure.code == ErrorCode::NotReady && failure.retryable
    ));
    assert_ne!(core.snapshot().scope().as_ref(), Some(&starting_scope));
}

#[path = "surface_reader_outbound_tests.rs"]
mod outbound_tests;
