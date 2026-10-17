//! Android Core process adapter host-boundary tests.

use super::*;
use infiltrator_application::core_application::{CoreApplication, ReadinessPolicy};
use infiltrator_application::core_capability_application::CoreCapabilityApplication;
#[cfg(test)]
use infiltrator_application::mtu_application::MtuApplication;
#[cfg(test)]
use infiltrator_application::offline_startup_application::OfflineStartupApplication;
use infiltrator_contract::android_control::core_launch::{
    CoreLaunchArguments, CoreLaunchBinary, CoreLaunchConfigurationIdentity, CoreLaunchPath,
    CoreLaunchSpec,
};
use infiltrator_contract::android_control::{
    AndroidChildProcessIdentity, AndroidCoreProcessExitEvidence, AndroidCoreProcessObservation,
    AndroidCoreProcessUnavailableReason, AndroidOwnedCoreProcess, VpnOwnerCoreReadback,
    VpnOwnerLifecycle, VpnOwnerReadiness, VpnOwnerSnapshot,
};
use infiltrator_contract::command::{CommandIntent, CommandResult};
use infiltrator_contract::core_capabilities::{
    CoreLifecycleOperation, CoreOperation, EngineBinaryTarget,
};
use infiltrator_contract::core_identity::CoreCommandRequest;
#[cfg(test)]
use infiltrator_contract::mtu::MtuProbeState;
#[cfg(test)]
use infiltrator_contract::offline_startup::LocalAssetStatus;
#[cfg(test)]
use infiltrator_contract::offline_startup::StartupNetworkPolicy;
#[cfg(test)]
use infiltrator_contract::offline_startup::StartupRemoteDependency;
use infiltrator_contract::profile_document_identity::{ProfileDocumentRevision, ProfileId};
use infiltrator_contract::vpn::VpnGeneration;
use infiltrator_contract::vpn::{
    PreparedVpnConfiguration, VpnConfigurationApplyOutcome, VpnOwnerStopReceipt, VpnStartOutcome,
    VpnStopTarget,
};
use infiltrator_platform::android_bridge::AndroidBridgeResult;
use infiltrator_ports::core_lifecycle::CoreLifecyclePort;
use lurker_core_contract::core_process::CoreProcessStartContext;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::{CoreGeneration, EngineId};
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_contract::lifecycle::CoreLifecycleSnapshot;
use lurker_core_contract::session::SessionToken;
use lurker_core_ports::core_process::{CoreProcessStartAdmission, CoreReadiness};
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
        context: &CoreProcessStartContext,
    ) -> AndroidBridgeResult<CoreProcessStartReceipt> {
        self.start_contexts
            .lock()
            .expect("start context lock")
            .push(context.clone());
        if let Ok(mut guard) = self.running.lock() {
            *guard = true;
        }
        Ok(CoreProcessStartReceipt::Started)
    }

    async fn core_stop(&self) -> AndroidBridgeResult<CoreProcessStopReceipt> {
        if let Ok(mut guard) = self.running.lock() {
            *guard = false;
        }
        Ok(CoreProcessStopReceipt::StoppedGracefully)
    }

    async fn core_process_observation(&self) -> AndroidBridgeResult<AndroidCoreProcessObservation> {
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

    async fn credential_get(
        &self,
        service: &str,
        key: &str,
    ) -> AndroidBridgeResult<Option<String>> {
        Ok(self
            .store
            .lock()
            .ok()
            .and_then(|g| g.get(&(service.to_string(), key.to_string())).cloned()))
    }

    async fn credential_set(
        &self,
        service: &str,
        key: &str,
        value: &str,
    ) -> AndroidBridgeResult<()> {
        if let Ok(mut guard) = self.store.lock() {
            guard.insert((service.to_string(), key.to_string()), value.to_string());
        }
        Ok(())
    }

    async fn credential_delete(&self, service: &str, key: &str) -> AndroidBridgeResult<()> {
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

    async fn vpn_start(&self) -> AndroidBridgeResult<VpnStartOutcome> {
        Ok(VpnStartOutcome::Running)
    }

    async fn vpn_apply_configuration(
        &self,
        _prepared: &PreparedVpnConfiguration,
    ) -> AndroidBridgeResult<VpnConfigurationApplyOutcome> {
        Ok(VpnConfigurationApplyOutcome::Accepted)
    }

    async fn vpn_stop(&self, _target: VpnStopTarget) -> AndroidBridgeResult<VpnOwnerStopReceipt> {
        Ok(VpnOwnerStopReceipt::OwnerUnavailable {
            failure: Failure::new(
                ErrorCode::NotReady,
                "fake VPN stop owner has no same-request readback",
                true,
            ),
        })
    }

    async fn vpn_owner_snapshot(&self) -> AndroidBridgeResult<VpnOwnerSnapshot> {
        Ok(VpnOwnerSnapshot {
            owner_session_id: "owner-session".to_owned(),
            revision: 1,
            vpn_generation: VpnGeneration::new(0),
            vpn_state: VpnOwnerLifecycle::Stopped,
            readiness: VpnOwnerReadiness::Stopped,
            resources: Vec::new(),
            foreground: false,
            mtu: None,
            route_count: 0,
            dns_servers: Vec::new(),
            ipv6: false,
            core: VpnOwnerCoreReadback::Available(CoreLifecycleSnapshot {
                identity: None,
                lifecycle: CoreLifecycle::Stopped,
                generation: 0,
                session_token: None,
                revision: 0,
                failure: None,
            }),
        })
    }
}

struct TestLaunchSpecSource {
    engine_id: EngineId,
}

impl TestLaunchSpecSource {
    fn mihomo() -> Self {
        Self {
            engine_id: EngineId::new("mihomo").expect("Mihomo engine ID"),
        }
    }
}

#[async_trait]
impl CoreLaunchSpecSource for TestLaunchSpecSource {
    async fn resolve(&self) -> result::Result<CoreLaunchSpec, PortError> {
        CoreLaunchSpec::new(
            CoreLaunchConfigurationIdentity {
                engine_id: self.engine_id.clone(),
                profile_id: ProfileId::new("default").expect("profile ID"),
                document_revision: ProfileDocumentRevision::new("sha256:runtime-test")
                    .expect("document revision"),
            },
            CoreLaunchBinary::MihomoBundled,
            PathBuf::from("/data"),
            CoreLaunchArguments::Mihomo {
                configuration_path: CoreLaunchPath::new("/data/config.yaml".to_owned())
                    .expect("absolute config path"),
            },
        )
        .map_err(|error| PortError::Failed(error.to_string()))
    }
}

#[tokio::test]
async fn test_adapter_core_cycle() {
    let bridge = TestBridge::new();
    let adapter = AndroidCoreProcessAdapter::new(
        EngineId::new("mihomo").expect("engine ID"),
        bridge.clone(),
        Arc::new(TestLaunchSpecSource::mihomo()),
    );
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
    let adapter = AndroidCoreProcessAdapter::new(
        EngineId::new("mihomo").expect("engine ID"),
        bridge.clone(),
        Arc::new(TestLaunchSpecSource::mihomo()),
    );
    let identity = adapter.core_identity().expect("Android Core identity");
    let requested = CoreProcessStartContext::managed(
        identity.clone(),
        CoreGeneration::new(1),
        SessionToken::new(17),
    );
    let foreign =
        CoreProcessStartContext::managed(identity, CoreGeneration::new(2), SessionToken::new(18));
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
    let adapter = AndroidCoreProcessAdapter::new(
        EngineId::new("mihomo").expect("engine ID"),
        bridge.clone(),
        Arc::new(TestLaunchSpecSource::mihomo()),
    );
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
async fn android_process_adapter_uses_typed_engine_identity_and_rejects_foreign_launch_spec() {
    let bridge = TestBridge::new();
    let sing_box_engine = EngineId::new("sing-box").expect("sing-box engine ID");
    let adapter = AndroidCoreProcessAdapter::new(
        sing_box_engine.clone(),
        bridge.clone(),
        Arc::new(TestLaunchSpecSource::mihomo()),
    );
    let identity = adapter.core_identity().expect("engine-scoped identity");
    assert_eq!(identity.engine, sing_box_engine);
    let context =
        CoreProcessStartContext::managed(identity, CoreGeneration::new(1), SessionToken::new(23));

    let error = adapter
        .start(context)
        .await
        .expect_err("foreign launch spec must fail before JNI");

    assert!(error.to_string().contains("launch specification engine"));
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
    let process = AndroidCoreProcessAdapter::new(
        EngineId::new("mihomo").expect("engine ID"),
        TestBridge::new(),
        Arc::new(TestLaunchSpecSource::mihomo()),
    );
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
