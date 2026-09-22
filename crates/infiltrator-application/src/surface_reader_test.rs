//! Behavior tests for the application surface reader.

use super::*;
use async_trait::async_trait;
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_contract::version::{CoreRelease, CoreReleaseChannel, CoreRollbackSnapshot};
use infiltrator_ports::application_runtime::{
    ApplicationFuture, ApplicationRuntime, ApplicationSleep,
};
use infiltrator_ports::core_process::{CoreProcess, CoreReadiness};
use infiltrator_ports::endpoint::{ControllerEndpoint, EndpointSource};
use infiltrator_ports::mtu_probe::MtuProbePort;
use infiltrator_ports::network_roaming::NetworkRoamingPort;
use infiltrator_ports::offline_startup::OfflineStartupPort;
use infiltrator_ports::port_conflict::PortConflictPort;
use infiltrator_ports::service_mode::ServiceModePort;
use infiltrator_ports::system_proxy::SystemProxyPort;
use infiltrator_ports::version::{VersionPort, VersionProgressSink};
use infiltrator_ports::vpn_service::VpnServicePort;
use std::sync::atomic::{AtomicUsize, Ordering};

struct TestRuntime;

impl ApplicationRuntime for TestRuntime {
    fn block_on(&self, future: ApplicationFuture) {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime")
            .block_on(future);
    }

    fn sleep(&self, duration: Duration) -> ApplicationSleep<'_> {
        Box::pin(tokio::time::sleep(duration))
    }
}

struct TestProcess;

#[async_trait]
impl CoreProcess for TestProcess {
    async fn start(&self) -> Result<(), PortError> {
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

struct TestReadiness;

#[async_trait]
impl CoreReadiness for TestReadiness {
    async fn probe(&self) -> Result<String, PortError> {
        Ok("http://127.0.0.1:9090".to_owned())
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
    async fn snapshot(
        &self,
    ) -> Result<infiltrator_contract::service_mode::ServiceModeSnapshot, PortError> {
        Ok(infiltrator_contract::service_mode::ServiceModeSnapshot {
            platform: infiltrator_contract::service_mode::ServiceModePlatform::LinuxPolkit,
            state: infiltrator_contract::service_mode::ServiceModeState::Ready,
        })
    }

    async fn prepare(
        &self,
    ) -> Result<infiltrator_contract::service_mode::ServiceModeSnapshot, PortError> {
        self.snapshot().await
    }
}

struct TestPortConflicts;

#[async_trait]
impl PortConflictPort for TestPortConflicts {
    async fn snapshot(
        &self,
    ) -> Result<infiltrator_contract::port_conflict::PortConflictSnapshot, PortError> {
        Ok(infiltrator_contract::port_conflict::PortConflictSnapshot {
            revision: 1,
            conflicts: vec![infiltrator_contract::port_conflict::PortConflict {
                binding: infiltrator_contract::port_conflict::PortBinding::Controller,
                port: 9090,
                available: false,
                owner_pid: Some(4242),
                owner_name: Some("mihomo".to_owned()),
                can_release: true,
            }],
        })
    }

    async fn repair(
        &self,
    ) -> Result<infiltrator_contract::port_conflict::PortConflictSnapshot, PortError> {
        Ok(infiltrator_contract::port_conflict::PortConflictSnapshot::default())
    }
}

struct TestOfflineStartup;

#[async_trait]
impl OfflineStartupPort for TestOfflineStartup {
    async fn validate_offline_startup(
        &self,
    ) -> Result<infiltrator_contract::offline_startup::OfflineStartupSnapshot, PortError> {
        Ok(
            infiltrator_contract::offline_startup::OfflineStartupSnapshot::ready(
                infiltrator_contract::offline_startup::LocalAssetStatus::Available,
            ),
        )
    }
}

struct TestMtu {
    calls: Arc<AtomicUsize>,
}

#[async_trait]
impl MtuProbePort for TestMtu {
    async fn probe_physical_mtu(
        &self,
    ) -> Result<infiltrator_contract::mtu::PhysicalMtuSnapshot, PortError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(infiltrator_contract::mtu::PhysicalMtuSnapshot {
            interface: "eth0".to_owned(),
            mtu: 1500,
        })
    }
}

struct TestSystemProxy {
    calls: Arc<AtomicUsize>,
}

struct TestNetworkRoaming {
    calls: Arc<AtomicUsize>,
}

struct TestVpn;

#[async_trait]
impl VpnServicePort for TestVpn {
    async fn request_start(
        &self,
    ) -> Result<infiltrator_contract::vpn::VpnSessionSnapshot, PortError> {
        Ok(infiltrator_contract::vpn::VpnSessionSnapshot {
            state: infiltrator_contract::vpn::VpnSessionState::Starting,
            foreground: true,
            ..Default::default()
        })
    }

    async fn prepare(
        &self,
        _configuration: infiltrator_contract::vpn::VpnConfiguration,
    ) -> Result<infiltrator_contract::vpn::VpnSessionSnapshot, PortError> {
        Ok(infiltrator_contract::vpn::VpnSessionSnapshot {
            state: infiltrator_contract::vpn::VpnSessionState::Starting,
            foreground: true,
            ..Default::default()
        })
    }

    async fn start(
        &self,
        request: infiltrator_contract::vpn::VpnStartRequest,
    ) -> Result<infiltrator_contract::vpn::VpnSessionSnapshot, PortError> {
        Ok(infiltrator_contract::vpn::VpnSessionSnapshot::running(
            1,
            request.mtu,
            request.routes.len(),
            request.dns_servers,
            request.ipv6,
            true,
        ))
    }

    async fn stop(&self) -> Result<infiltrator_contract::vpn::VpnSessionSnapshot, PortError> {
        Ok(infiltrator_contract::vpn::VpnSessionSnapshot {
            state: infiltrator_contract::vpn::VpnSessionState::Stopped,
            ..Default::default()
        })
    }

    async fn revoke(&self) -> Result<infiltrator_contract::vpn::VpnSessionSnapshot, PortError> {
        Ok(infiltrator_contract::vpn::VpnSessionSnapshot {
            state: infiltrator_contract::vpn::VpnSessionState::Revoked,
            ..Default::default()
        })
    }

    async fn snapshot(&self) -> Result<infiltrator_contract::vpn::VpnSessionSnapshot, PortError> {
        Ok(infiltrator_contract::vpn::VpnSessionSnapshot::running(
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
    async fn observe(
        &self,
    ) -> Result<infiltrator_contract::network_roaming::NetworkObservation, PortError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(infiltrator_contract::network_roaming::NetworkObservation {
            interfaces: vec![
                infiltrator_contract::network_roaming::NetworkInterfaceSnapshot {
                    name: "eth0".to_owned(),
                    kind: infiltrator_contract::network_roaming::NetworkInterfaceKind::Ethernet,
                    is_up: true,
                    is_default_gateway: true,
                    gateway_ip: Some("192.0.2.1".to_owned()),
                    ip_addresses: vec!["192.0.2.10/24".to_owned()],
                    mtu: Some(1500),
                    metric: Some(100),
                    dns_servers: Vec::new(),
                },
            ],
            observed_at_epoch_ms: Some(1),
        })
    }

    async fn repair(
        &self,
        _request: infiltrator_contract::network_roaming::NetworkRoamingRepairRequest,
    ) -> Result<infiltrator_contract::network_roaming::NetworkRoamingRepairResult, PortError> {
        Ok(
            infiltrator_contract::network_roaming::NetworkRoamingRepairResult {
                route_generation: 1,
                detail: "test readback".to_owned(),
            },
        )
    }
}

#[async_trait]
impl SystemProxyPort for TestSystemProxy {
    async fn snapshot(
        &self,
    ) -> Result<infiltrator_contract::system_proxy::SystemProxyObservation, PortError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(infiltrator_contract::system_proxy::SystemProxyObservation {
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

struct TestVersions {
    calls: Arc<AtomicUsize>,
}

#[async_trait]
impl VersionPort for TestVersions {
    async fn list_installed(
        &self,
    ) -> Result<Vec<infiltrator_contract::version::InstalledCoreVersion>, PortError> {
        Ok(Vec::new())
    }

    async fn latest(&self, channel: CoreReleaseChannel) -> Result<CoreRelease, PortError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(CoreRelease {
            version: format!("{}-v1.0.0", channel.as_str()),
            release_date: "2026-09-05".to_owned(),
        })
    }

    async fn list_releases(
        &self,
        _limit: usize,
    ) -> Result<Vec<infiltrator_contract::version::CoreReleaseSummary>, PortError> {
        Ok(Vec::new())
    }

    async fn install(
        &self,
        _version: String,
        _progress: Arc<dyn VersionProgressSink>,
    ) -> Result<(), PortError> {
        Ok(())
    }

    async fn activate(&self, _version: &str) -> Result<(), PortError> {
        Ok(())
    }

    async fn uninstall(&self, _version: &str) -> Result<(), PortError> {
        Ok(())
    }

    async fn rollback_snapshot(&self) -> Result<CoreRollbackSnapshot, PortError> {
        Ok(CoreRollbackSnapshot {
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
    let reader = ApplicationSurfaceReader::new(core, SurfaceKind::BevyDesktop, HostKind::Desktop)
        .with_versions(VersionApplication::new(Arc::new(TestVersions {
            calls: calls.clone(),
        })))
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
        .with_network_roaming(NetworkRoamingApplication::new(
            Arc::new(TestNetworkRoaming {
                calls: network_calls.clone(),
            }),
            None,
        ))
        .with_vpn(VpnServiceApplication::new(Arc::new(TestVpn)));

    let first = reader.read().await.expect("first surface read");
    let second = reader.read().await.expect("cached surface read");
    assert_eq!(first.versions.channels.len(), 3);
    assert_eq!(first.versions.revision, 1);
    assert_eq!(second.versions.revision, 1);
    assert_eq!(calls.load(Ordering::SeqCst), 3);
    assert_eq!(first.versions.rollback.target.as_deref(), Some("v1.19.29"));
    assert_eq!(
        first.controller_auth.status,
        infiltrator_contract::controller::ControllerAuthStatus::Secured
    );
    assert_eq!(
        first.service_mode.state,
        infiltrator_contract::service_mode::ServiceModeState::Ready
    );
    assert!(first.port_conflicts.has_conflicts());
    assert_eq!(first.port_conflicts.conflicts[0].owner_pid, Some(4242));
    assert!(first.offline_startup.is_offline_startable());
    assert_eq!(
        first.offline_startup.geoip,
        infiltrator_contract::offline_startup::LocalAssetStatus::Available
    );
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
    assert!(first.vpn.is_running());
    assert_eq!(first.vpn.route_count, 2);
    assert!(matches!(
        first.privileged_network.state,
        infiltrator_contract::privileged_network::PrivilegedNetworkState::Unsupported { .. }
    ));
    assert!(first.versions.channels.iter().all(|channel| matches!(
        channel.status,
        infiltrator_contract::version::CoreChannelStatus::Ready { .. }
    )));
}

/// DUAL-11-14: the shared reader publishes the three rules-workspace JSON
/// documents, in the shared section order, and omits a section the host cannot
/// read instead of publishing an empty document for it.
#[test]
fn rules_json_documents_cover_the_shared_sections_and_omit_unreadable_ones() {
    use infiltrator_contract::rules_workspace::{RulesJsonDocumentSnapshot, RulesJsonSection};

    let rule_providers: infiltrator_domain::rules::RuleProviders =
        serde_json::from_value(serde_json::json!({
            "ads": {"type": "inline", "behavior": "domain", "payload": ["ads.com"]}
        }))
        .expect("rule providers");
    let proxy_providers: infiltrator_domain::proxy_providers::ProxyProviders =
        serde_json::from_value(serde_json::json!({
            "sub": {"type": "http", "url": "https://example.com/proxies.yaml"}
        }))
        .expect("proxy providers");
    let sniffer = serde_json::json!({"enable": true});

    let documents = super::rules_json_documents(
        Some(rule_providers.clone()),
        Some(proxy_providers.clone()),
        Some(sniffer.clone()),
    );
    assert_eq!(documents.len(), RulesJsonSection::ALL.len());
    assert_eq!(
        documents
            .iter()
            .map(|document| document.section)
            .collect::<Vec<_>>(),
        RulesJsonSection::ALL.to_vec()
    );

    // Each document round-trips back to the value it was serialised from, so
    // the Bevy editor edits exactly the text the shared application owns.
    for document in &documents {
        let value: serde_json::Value =
            serde_json::from_str(&document.json).expect("document is valid JSON");
        match document.section {
            RulesJsonSection::RuleProviders => {
                assert_eq!(value, serde_json::to_value(&rule_providers).unwrap());
            }
            RulesJsonSection::ProxyProviders => {
                assert_eq!(value, serde_json::to_value(&proxy_providers).unwrap());
            }
            RulesJsonSection::Sniffer => assert_eq!(value, sniffer),
        }
    }

    // A section the host cannot read is omitted, not fabricated.
    let partial = super::rules_json_documents(None, None, Some(sniffer));
    assert_eq!(
        partial,
        vec![RulesJsonDocumentSnapshot {
            section: RulesJsonSection::Sniffer,
            json: partial[0].json.clone(),
        }]
    );
    assert!(partial[0].json.contains("enable"));
}
