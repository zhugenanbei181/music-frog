use async_trait::async_trait;
use infiltrator_contract::android_control::{AndroidCoreProcessObservation, CoreLaunchSpec};
use infiltrator_contract::capability::{
    Availability, Capability, CapabilitySnapshot, CapabilityStatus,
};
use infiltrator_contract::core_identity::{CoreIdentity, CoreInstanceId, EngineId};
use infiltrator_contract::mtu::PhysicalMtuSnapshot;
use infiltrator_contract::offline_startup::OfflineStartupSnapshot;
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_contract::surface::HostKind;
use infiltrator_ports::capability_provider::CapabilityProvider;
use infiltrator_ports::core_process::{
    CoreProcess, CoreProcessStartAdmission, CoreProcessStartContext,
};
use infiltrator_ports::data_dir::DataDirProvider;
use infiltrator_ports::error::PortError;
use infiltrator_ports::mtu_probe::MtuProbePort;
use infiltrator_ports::offline_startup::OfflineStartupPort;
use infiltrator_ports::secure_store::SecureStore;
use mihomo_api::error::MihomoError;
#[cfg(test)]
use mihomo_api::error::Result;
use mihomo_platform::android_bridge::AndroidBridge;
#[cfg(test)]
use mihomo_platform::android_bridge::{CoreProcessStartReceipt, CoreProcessStopReceipt};
use std::path::PathBuf;
use std::result;
use std::sync::Arc;

#[derive(Clone)]
pub struct AndroidBridgeAdapter<B> {
    bridge: B,
}

impl<B> AndroidBridgeAdapter<B> {
    pub fn new(bridge: B) -> Self {
        Self { bridge }
    }
}

#[async_trait]
pub(crate) trait CoreLaunchSpecSource: Send + Sync {
    async fn resolve(&self) -> result::Result<CoreLaunchSpec, PortError>;
}

#[derive(Clone)]
pub(crate) struct AndroidCoreProcessAdapter<B> {
    bridge: B,
    launch_spec_source: Arc<dyn CoreLaunchSpecSource>,
}

impl<B> AndroidCoreProcessAdapter<B> {
    pub(crate) fn new(bridge: B, launch_spec_source: Arc<dyn CoreLaunchSpecSource>) -> Self {
        Self {
            bridge,
            launch_spec_source,
        }
    }
}

pub fn android_bridge_adapter<B>(bridge: B) -> AndroidBridgeAdapter<B>
where
    B: AndroidBridge,
{
    AndroidBridgeAdapter::new(bridge)
}

#[async_trait]
impl<B> OfflineStartupPort for AndroidBridgeAdapter<B>
where
    B: AndroidBridge,
{
    async fn validate_offline_startup(&self) -> result::Result<OfflineStartupSnapshot, PortError> {
        // The Android host owns the APK ABI binary and sandbox paths. Rust
        // consumes its typed local proof and never performs a remote probe.
        Ok(self.bridge.offline_startup_snapshot())
    }
}

#[async_trait]
impl<B> MtuProbePort for AndroidBridgeAdapter<B>
where
    B: AndroidBridge,
{
    async fn probe_physical_mtu(&self) -> result::Result<PhysicalMtuSnapshot, PortError> {
        self.bridge
            .physical_mtu()
            .map(|mtu| PhysicalMtuSnapshot {
                interface: "android-active-link".to_owned(),
                mtu,
            })
            .ok_or_else(|| {
                PortError::unsupported(
                    Capability::Tun,
                    "Android native bridge does not expose physical MTU",
                )
            })
    }
}

fn map_port_error(error: MihomoError) -> PortError {
    match error {
        MihomoError::Io(error) => PortError::Io(error.to_string()),
        MihomoError::Http(error) => PortError::Network(error.to_string()),
        MihomoError::WebSocket(error) => PortError::Network(error.to_string()),
        MihomoError::NotFound(message) => PortError::NotFound(message),
        other => PortError::Failed(other.to_string()),
    }
}

#[async_trait]
impl<B> CoreProcess for AndroidCoreProcessAdapter<B>
where
    B: AndroidBridge,
{
    fn core_identity(&self) -> Option<CoreIdentity> {
        Some(CoreIdentity::new(
            EngineId::new("mihomo").ok()?,
            CoreInstanceId::new("android-active").ok()?,
            None,
        ))
    }

    async fn prepare_start(
        &self,
        context: &CoreProcessStartContext,
    ) -> result::Result<CoreProcessStartAdmission, PortError> {
        let expected_identity = self
            .core_identity()
            .ok_or_else(|| PortError::Failed("Android Core identity is unavailable".to_owned()))?;
        validate_android_start_context(context, &expected_identity)?;
        match self
            .bridge
            .core_process_observation()
            .await
            .map_err(map_port_error)?
        {
            AndroidCoreProcessObservation::NeverStarted => {
                Ok(CoreProcessStartAdmission::LaunchNewProcess)
            }
            AndroidCoreProcessObservation::Running { owned } if owned.context == *context => {
                Ok(CoreProcessStartAdmission::ReuseExactOwnedProcess)
            }
            AndroidCoreProcessObservation::Running { .. } => Err(PortError::Failed(
                "Android Mihomo child belongs to a different Core session".to_owned(),
            )),
            AndroidCoreProcessObservation::Exited { .. } => Err(PortError::Failed(
                "Android Mihomo child exit awaits owner recovery".to_owned(),
            )),
            AndroidCoreProcessObservation::Unavailable { reason } => Err(PortError::Failed(
                format!("Android Core process observation is unavailable: {reason:?}"),
            )),
        }
    }

    async fn start(
        &self,
        context: infiltrator_ports::core_process::CoreProcessStartContext,
    ) -> result::Result<(), PortError> {
        match self.prepare_start(&context).await? {
            CoreProcessStartAdmission::LaunchNewProcess => {}
            CoreProcessStartAdmission::ReuseExactOwnedProcess => return Ok(()),
        }
        let launch_spec = self.launch_spec_source.resolve().await?;
        self.bridge
            .core_start(&launch_spec, &context)
            .await
            .map_err(map_port_error)?
            .into_port_result()
    }

    async fn stop(&self) -> result::Result<(), PortError> {
        self.bridge
            .core_stop()
            .await
            .map_err(map_port_error)?
            .into_port_result()
    }

    async fn status(&self) -> result::Result<CoreLifecycle, PortError> {
        match self
            .bridge
            .core_process_observation()
            .await
            .map_err(map_port_error)?
        {
            AndroidCoreProcessObservation::NeverStarted
            | AndroidCoreProcessObservation::Exited { .. } => Ok(CoreLifecycle::Stopped),
            AndroidCoreProcessObservation::Running { .. } => Ok(CoreLifecycle::Running),
            AndroidCoreProcessObservation::Unavailable { reason } => Err(PortError::Failed(
                format!("Android Core process observation is unavailable: {reason:?}"),
            )),
        }
    }

    fn controller_endpoint(&self) -> Option<String> {
        None
    }
}

fn validate_android_start_context(
    context: &CoreProcessStartContext,
    expected_identity: &CoreIdentity,
) -> result::Result<(), PortError> {
    match context {
        CoreProcessStartContext::Managed {
            identity,
            generation,
            session_token,
        } if identity.same_instance(expected_identity)
            && generation.value() > 0
            && session_token.is_valid() =>
        {
            Ok(())
        }
        CoreProcessStartContext::Managed { .. } => Err(PortError::Failed(
            "Core start context targets a different Android process".to_owned(),
        )),
        CoreProcessStartContext::Standalone { .. } => Err(PortError::Failed(
            "Android Core start requires a managed owner session".to_owned(),
        )),
    }
}

#[async_trait]
impl<B> SecureStore for AndroidBridgeAdapter<B>
where
    B: AndroidBridge,
{
    async fn get(&self, namespace: &str, key: &str) -> result::Result<Option<String>, PortError> {
        self.bridge
            .credential_get(namespace, key)
            .await
            .map_err(map_port_error)
    }

    async fn set(&self, namespace: &str, key: &str, value: &str) -> result::Result<(), PortError> {
        self.bridge
            .credential_set(namespace, key, value)
            .await
            .map_err(map_port_error)
    }

    async fn delete(&self, namespace: &str, key: &str) -> result::Result<(), PortError> {
        self.bridge
            .credential_delete(namespace, key)
            .await
            .map_err(map_port_error)
    }
}

impl<B> DataDirProvider for AndroidBridgeAdapter<B>
where
    B: AndroidBridge,
{
    fn data_dir(&self) -> Option<PathBuf> {
        self.bridge.data_dir()
    }

    fn cache_dir(&self) -> Option<PathBuf> {
        self.bridge.cache_dir()
    }
}

impl<B> CapabilityProvider for AndroidBridgeAdapter<B>
where
    B: AndroidBridge,
{
    fn host_kind(&self) -> HostKind {
        HostKind::Android
    }

    fn capabilities(&self) -> CapabilitySnapshot {
        CapabilitySnapshot::new(
            HostKind::Android,
            1,
            vec![
                CapabilityStatus {
                    capability: Capability::CoreLifecycle,
                    availability: Availability::Supported,
                },
                CapabilityStatus {
                    capability: Capability::Tun,
                    availability: Availability::Supported,
                },
                CapabilityStatus {
                    capability: Capability::SystemProxy,
                    availability: Availability::Unsupported {
                        reason: "Android VpnService owns proxy routing".to_string(),
                    },
                },
                CapabilityStatus {
                    capability: Capability::LanAccessControl,
                    availability: Availability::Supported,
                },
                CapabilityStatus {
                    capability: Capability::Ipv6Routing,
                    availability: Availability::Supported,
                },
                CapabilityStatus {
                    capability: Capability::UwpLoopback,
                    availability: Availability::Unsupported {
                        reason: "Windows AppContainer loopback is not available on Android"
                            .to_owned(),
                    },
                },
                CapabilityStatus {
                    capability: Capability::PacService,
                    availability: Availability::Unsupported {
                        reason: "Android uses VpnService instead of desktop PAC injection"
                            .to_owned(),
                    },
                },
                CapabilityStatus {
                    capability: Capability::NetworkRoaming,
                    availability: Availability::Unsupported {
                        reason: "Android native VpnService route callbacks are not exposed"
                            .to_owned(),
                    },
                },
                CapabilityStatus {
                    capability: Capability::VpnService,
                    availability: Availability::Supported,
                },
                CapabilityStatus {
                    capability: Capability::Logs,
                    availability: Availability::Supported,
                },
                CapabilityStatus {
                    capability: Capability::CoreVersionInstall,
                    availability: Availability::Unsupported {
                        reason: "core binaries are delivered with the APK ABI".to_string(),
                    },
                },
            ],
        )
    }
}

pub struct AndroidRuntime<B>
where
    B: AndroidBridge,
{
    adapter: AndroidBridgeAdapter<B>,
}

impl<B> AndroidRuntime<B>
where
    B: AndroidBridge,
{
    pub fn new(adapter: AndroidBridgeAdapter<B>) -> Self {
        Self { adapter }
    }

    pub fn credential_store(&self) -> &dyn SecureStore {
        &self.adapter
    }

    pub fn data_dirs(&self) -> &dyn DataDirProvider {
        &self.adapter
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_application::core_application::{CoreApplication, ReadinessPolicy};
    use infiltrator_application::core_capability_application::CoreCapabilityApplication;
    #[cfg(test)]
    use infiltrator_application::mtu_application::MtuApplication;
    #[cfg(test)]
    use infiltrator_application::offline_startup_application::OfflineStartupApplication;
    use infiltrator_contract::android_control::{
        AndroidChildProcessIdentity, AndroidCoreProcessExitEvidence, AndroidCoreProcessObservation,
        AndroidCoreProcessUnavailableReason, AndroidOwnedCoreProcess,
        CoreLaunchConfigurationIdentity, CoreLaunchSpec, VpnOwnerCoreReadback, VpnOwnerLifecycle,
        VpnOwnerReadiness, VpnOwnerSnapshot,
    };
    use infiltrator_contract::command::{CommandIntent, CommandResult};
    use infiltrator_contract::core_capabilities::{
        CoreLifecycleOperation, CoreOperation, EngineBinaryTarget,
    };
    use infiltrator_contract::core_identity::CoreCommandRequest;
    use infiltrator_contract::core_identity::CoreGeneration;
    #[cfg(test)]
    use infiltrator_contract::mtu::MtuProbeState;
    #[cfg(test)]
    use infiltrator_contract::offline_startup::LocalAssetStatus;
    #[cfg(test)]
    use infiltrator_contract::offline_startup::StartupNetworkPolicy;
    #[cfg(test)]
    use infiltrator_contract::offline_startup::StartupRemoteDependency;
    use infiltrator_contract::session::SessionToken;
    use infiltrator_contract::snapshot::CoreLifecycle;
    use infiltrator_contract::vpn::{
        PreparedVpnConfiguration, VpnConfigurationApplyOutcome, VpnOwnerStopReceipt,
        VpnStartOutcome, VpnStopTarget,
    };
    use infiltrator_ports::core_lifecycle::CoreLifecyclePort;
    use infiltrator_ports::core_process::{CoreProcessStartAdmission, CoreReadiness};
    use mihomo_api::core_capability_provider::MihomoCoreCapabilityProvider;
    use std::collections::HashMap;
    use std::sync::Mutex;
    #[cfg(test)]
    use std::time;

    #[derive(Clone)]
    struct TestBridge {
        running: Arc<Mutex<bool>>,
        start_contexts: Arc<Mutex<Vec<CoreProcessStartContext>>>,
        process_observation: Arc<Mutex<TestProcessObservation>>,
        store: Arc<Mutex<HashMap<(String, String), String>>>,
        data_dir: Option<PathBuf>,
        cache_dir: Option<PathBuf>,
    }

    #[derive(Clone)]
    enum TestProcessObservation {
        Automatic,
        Fixed(AndroidCoreProcessObservation),
    }

    impl TestBridge {
        fn new() -> Self {
            Self {
                running: Arc::new(Mutex::new(false)),
                start_contexts: Arc::new(Mutex::new(Vec::new())),
                process_observation: Arc::new(Mutex::new(TestProcessObservation::Automatic)),
                store: Arc::new(Mutex::new(HashMap::new())),
                data_dir: Some(PathBuf::from("data")),
                cache_dir: Some(PathBuf::from("cache")),
            }
        }

        fn set_process_observation(&self, observation: AndroidCoreProcessObservation) {
            *self
                .process_observation
                .lock()
                .expect("process observation lock") = TestProcessObservation::Fixed(observation);
        }
    }

    #[async_trait]
    impl AndroidBridge for TestBridge {
        async fn core_start(
            &self,
            _launch_spec: &CoreLaunchSpec,
            context: &infiltrator_ports::core_process::CoreProcessStartContext,
        ) -> Result<CoreProcessStartReceipt> {
            self.start_contexts
                .lock()
                .expect("start context lock")
                .push(context.clone());
            if let Ok(mut guard) = self.running.lock() {
                *guard = true;
            }
            Ok(CoreProcessStartReceipt::Started)
        }

        async fn core_stop(&self) -> Result<CoreProcessStopReceipt> {
            if let Ok(mut guard) = self.running.lock() {
                *guard = false;
            }
            Ok(CoreProcessStopReceipt::StoppedGracefully)
        }

        async fn core_process_observation(&self) -> Result<AndroidCoreProcessObservation> {
            match self
                .process_observation
                .lock()
                .expect("process observation lock")
                .clone()
            {
                TestProcessObservation::Fixed(observation) => return Ok(observation),
                TestProcessObservation::Automatic => {}
            }
            let running = self.running.lock().map(|guard| *guard).unwrap_or(false);
            if !running {
                return Ok(AndroidCoreProcessObservation::NeverStarted);
            }
            let context = self
                .start_contexts
                .lock()
                .expect("start context lock")
                .last()
                .cloned();
            match context {
                Some(context) => Ok(AndroidCoreProcessObservation::Running {
                    owned: AndroidOwnedCoreProcess {
                        context,
                        child: AndroidChildProcessIdentity {
                            pid: 321,
                            start_time_ticks: 987_654,
                        },
                    },
                }),
                None => Ok(AndroidCoreProcessObservation::Unavailable {
                    reason: AndroidCoreProcessUnavailableReason::OwnershipRecordCorrupt,
                }),
            }
        }

        async fn credential_get(&self, service: &str, key: &str) -> Result<Option<String>> {
            Ok(self
                .store
                .lock()
                .ok()
                .and_then(|g| g.get(&(service.to_string(), key.to_string())).cloned()))
        }

        async fn credential_set(&self, service: &str, key: &str, value: &str) -> Result<()> {
            if let Ok(mut guard) = self.store.lock() {
                guard.insert((service.to_string(), key.to_string()), value.to_string());
            }
            Ok(())
        }

        async fn credential_delete(&self, service: &str, key: &str) -> Result<()> {
            if let Ok(mut guard) = self.store.lock() {
                guard.remove(&(service.to_string(), key.to_string()));
            }
            Ok(())
        }

        fn data_dir(&self) -> Option<PathBuf> {
            self.data_dir.clone()
        }

        fn cache_dir(&self) -> Option<PathBuf> {
            self.cache_dir.clone()
        }

        fn offline_startup_snapshot(&self) -> OfflineStartupSnapshot {
            OfflineStartupSnapshot::ready(LocalAssetStatus::Available)
        }

        async fn vpn_start(&self) -> Result<VpnStartOutcome> {
            Ok(VpnStartOutcome::Running)
        }

        async fn vpn_apply_configuration(
            &self,
            _prepared: &PreparedVpnConfiguration,
        ) -> Result<VpnConfigurationApplyOutcome> {
            Ok(VpnConfigurationApplyOutcome::Accepted)
        }

        async fn vpn_stop(&self, _target: VpnStopTarget) -> Result<VpnOwnerStopReceipt> {
            Ok(VpnOwnerStopReceipt::OwnerUnavailable {
                failure: infiltrator_contract::error::Failure::new(
                    infiltrator_contract::error::ErrorCode::NotReady,
                    "fake VPN stop owner has no same-request readback",
                    true,
                ),
            })
        }

        async fn vpn_owner_snapshot(&self) -> Result<VpnOwnerSnapshot> {
            Ok(VpnOwnerSnapshot {
                owner_session_id: "owner-session".to_owned(),
                revision: 1,
                vpn_generation: infiltrator_contract::vpn::VpnGeneration::new(0),
                vpn_state: VpnOwnerLifecycle::Stopped,
                readiness: VpnOwnerReadiness::Stopped,
                resources: Vec::new(),
                foreground: false,
                mtu: None,
                route_count: 0,
                dns_servers: Vec::new(),
                ipv6: false,
                core: VpnOwnerCoreReadback::Available(
                    infiltrator_contract::snapshot::CoreLifecycleSnapshot {
                        identity: None,
                        lifecycle: CoreLifecycle::Stopped,
                        generation: 0,
                        session_token: None,
                        revision: 0,
                        failure: None,
                    },
                ),
            })
        }
    }

    struct TestLaunchSpecSource;

    #[async_trait]
    impl CoreLaunchSpecSource for TestLaunchSpecSource {
        async fn resolve(&self) -> result::Result<CoreLaunchSpec, PortError> {
            CoreLaunchSpec::new(
                CoreLaunchConfigurationIdentity {
                    engine_id: EngineId::new("mihomo").expect("engine ID"),
                    profile_id: infiltrator_contract::profile_document_identity::ProfileId::new(
                        "default",
                    )
                    .expect("profile ID"),
                    document_revision: infiltrator_contract::profile_document_identity::ProfileDocumentRevision::new(
                        "sha256:runtime-test",
                    )
                    .expect("document revision"),
                },
                PathBuf::from("/data"),
                vec!["-d".to_owned(), "/data".to_owned()],
            )
            .map_err(|error| PortError::Failed(error.to_string()))
        }
    }

    #[tokio::test]
    async fn test_adapter_core_cycle() {
        let bridge = TestBridge::new();
        let adapter =
            AndroidCoreProcessAdapter::new(bridge.clone(), Arc::new(TestLaunchSpecSource));
        assert!(matches!(
            CoreProcess::status(&adapter).await.unwrap(),
            CoreLifecycle::Stopped
        ));
        let start_context = CoreProcessStartContext::managed(
            adapter.core_identity().expect("Android Core identity"),
            CoreGeneration::new(1),
            SessionToken::new(1),
        );
        assert_eq!(
            adapter.prepare_start(&start_context).await.unwrap(),
            CoreProcessStartAdmission::LaunchNewProcess
        );
        adapter
            .start(start_context.clone())
            .await
            .expect("start ok");
        assert_eq!(
            bridge
                .start_contexts
                .lock()
                .expect("start context lock")
                .as_slice(),
            [start_context.clone()],
        );
        assert_eq!(
            adapter.prepare_start(&start_context).await.unwrap(),
            CoreProcessStartAdmission::ReuseExactOwnedProcess
        );
        assert!(matches!(
            CoreProcess::status(&adapter).await.unwrap(),
            CoreLifecycle::Running
        ));
        adapter.stop().await.expect("stop ok");
        assert!(matches!(
            CoreProcess::status(&adapter).await.unwrap(),
            CoreLifecycle::Stopped
        ));
    }

    #[tokio::test]
    async fn android_start_admission_rejects_foreign_exited_and_unavailable_child_proofs() {
        let bridge = TestBridge::new();
        let adapter =
            AndroidCoreProcessAdapter::new(bridge.clone(), Arc::new(TestLaunchSpecSource));
        let identity = adapter.core_identity().expect("Android Core identity");
        let requested = CoreProcessStartContext::managed(
            identity.clone(),
            CoreGeneration::new(1),
            SessionToken::new(17),
        );
        let foreign = CoreProcessStartContext::managed(
            identity,
            CoreGeneration::new(2),
            SessionToken::new(18),
        );
        let owned = AndroidOwnedCoreProcess {
            context: foreign.clone(),
            child: AndroidChildProcessIdentity {
                pid: 321,
                start_time_ticks: 987_654,
            },
        };

        bridge.set_process_observation(AndroidCoreProcessObservation::Running { owned });
        assert!(adapter.prepare_start(&requested).await.is_err());
        bridge.set_process_observation(AndroidCoreProcessObservation::Exited {
            owned: AndroidOwnedCoreProcess {
                context: requested.clone(),
                child: AndroidChildProcessIdentity {
                    pid: 321,
                    start_time_ticks: 987_654,
                },
            },
            evidence: AndroidCoreProcessExitEvidence::ProcessHandleExited,
        });
        assert!(adapter.prepare_start(&requested).await.is_err());
        bridge.set_process_observation(AndroidCoreProcessObservation::Unavailable {
            reason: AndroidCoreProcessUnavailableReason::ProcessInspectionUnavailable,
        });
        assert!(adapter.prepare_start(&requested).await.is_err());
        assert!(
            bridge
                .start_contexts
                .lock()
                .expect("start context lock")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn android_process_adapter_rejects_standalone_context_before_host_call() {
        let bridge = TestBridge::new();
        let adapter =
            AndroidCoreProcessAdapter::new(bridge.clone(), Arc::new(TestLaunchSpecSource));
        let context = CoreProcessStartContext::standalone(
            adapter.core_identity().expect("Android Core identity"),
        );

        let error = adapter
            .start(context)
            .await
            .expect_err("Android requires owner scope");
        assert!(error.to_string().contains("managed owner session"));
        assert!(
            bridge
                .start_contexts
                .lock()
                .expect("start context lock")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn test_adapter_credentials() {
        let adapter = AndroidBridgeAdapter::new(TestBridge::new());
        let value = adapter.get("svc", "key").await.expect("get ok");
        assert!(value.is_none());
        adapter.set("svc", "key", "secret").await.expect("set ok");
        let value = adapter.get("svc", "key").await.expect("get ok");
        assert_eq!(value, Some("secret".to_string()));
        adapter.delete("svc", "key").await.expect("delete ok");
        let value = adapter.get("svc", "key").await.expect("get ok");
        assert!(value.is_none());
    }

    #[tokio::test]
    async fn test_runtime_accessors() {
        let adapter = AndroidBridgeAdapter::new(TestBridge::new());
        assert!(
            adapter
                .capabilities()
                .supports(Capability::LanAccessControl)
        );
        assert!(adapter.capabilities().supports(Capability::Ipv6Routing));
        assert!(!adapter.capabilities().supports(Capability::UwpLoopback));
        assert!(!adapter.capabilities().supports(Capability::NetworkRoaming));
        assert!(adapter.capabilities().supports(Capability::VpnService));
        assert!(adapter.capabilities().supports(Capability::Logs));
        assert!(matches!(
            adapter
                .capabilities()
                .availability(Capability::NetworkRoaming),
            Availability::Unsupported { .. }
        ));
        let runtime = AndroidRuntime::new(adapter);
        assert_eq!(runtime.data_dirs().data_dir(), Some(PathBuf::from("data")));
        assert_eq!(
            runtime.data_dirs().cache_dir(),
            Some(PathBuf::from("cache"))
        );
    }

    #[tokio::test]
    async fn android_host_exposes_offline_first_startup_evidence() {
        let adapter = AndroidBridgeAdapter::new(TestBridge::new());
        let snapshot = OfflineStartupApplication::new(Arc::new(adapter))
            .snapshot()
            .await;
        assert!(snapshot.is_offline_startable());
        assert_eq!(snapshot.policy, StartupNetworkPolicy::OfflineFirst);
        assert_eq!(
            snapshot.remote_dependency,
            StartupRemoteDependency::Optional
        );
    }

    #[tokio::test]
    async fn android_host_keeps_mtu_probe_unsupported_without_native_link_metrics() {
        let adapter = AndroidBridgeAdapter::new(TestBridge::new());
        let snapshot = MtuApplication::new(Arc::new(adapter)).probe().await;
        assert_eq!(snapshot.state, MtuProbeState::Unsupported);
    }

    struct ReadyProbe;

    #[async_trait]
    impl CoreReadiness for ReadyProbe {
        async fn probe(&self) -> result::Result<String, PortError> {
            Ok("http://127.0.0.1:9090".to_string())
        }
    }

    #[tokio::test]
    async fn android_host_composition_preserves_hot_reload_session_identity() {
        let process =
            AndroidCoreProcessAdapter::new(TestBridge::new(), Arc::new(TestLaunchSpecSource));
        let runtime = infiltrator_composition::tokio_application_runtime().expect("runtime");
        let app = CoreApplication::new_with_policy(
            Arc::new(process),
            Arc::new(ReadyProbe),
            ReadinessPolicy {
                timeout: time::Duration::from_secs(1),
                poll_interval: time::Duration::from_millis(1),
            },
            runtime,
        );
        let binary_target = EngineBinaryTarget::current();
        let host = binary_target.host_kind().unwrap_or(HostKind::Desktop);
        app.install_core_capability_application(CoreCapabilityApplication::new(Arc::new(
            MihomoCoreCapabilityProvider::new(
                host,
                [CoreOperation::Lifecycle(CoreLifecycleOperation::Start)],
            ),
        )))
        .expect("Android lifecycle test capability installs before use");

        let request = CoreCommandRequest {
            scope: app.snapshot().scope().expect("Android owner scope"),
            intent: CommandIntent::StartCore,
        };
        assert!(matches!(
            app.execute_scoped(request).await.result,
            CommandResult::Completed { .. }
        ));
        let before = app.snapshot();
        let token = CoreLifecyclePort::begin_reload(&app).expect("begin Android reload");
        CoreLifecyclePort::complete_reload(&app, token).expect("complete Android reload");
        let after = app.snapshot();
        assert_eq!(after.generation, before.generation);
        assert_eq!(after.session_token, before.session_token);
        assert_eq!(after.lifecycle, CoreLifecycle::Running);
    }
}
