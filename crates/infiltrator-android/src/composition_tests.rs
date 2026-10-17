//! Android owner profile-command composition behavior.
//! test-intent: behavior

use super::AndroidCoreComposition;
use async_trait::async_trait;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::overview::UnavailableOverviewReader;
use infiltrator_composition::engine_backend_factory::EngineBackendFactoryRegistry;
use infiltrator_contract::android_control::core_launch::CoreLaunchSpec;
use infiltrator_contract::android_control::{
    AndroidChildProcessIdentity, AndroidCoreProcessObservation, AndroidHostProcessRole,
    AndroidOwnedCoreProcess, AndroidProductCommand, AndroidProductCommandRequest,
    VpnOwnerCoreReadback, VpnOwnerLifecycle, VpnOwnerReadiness, VpnOwnerSnapshot,
};
use infiltrator_contract::command::{CommandIntent, CommandResult};
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::core_capabilities::{
    AdapterCapabilityDecision, AdapterRevision, ConnectionOperation, CoreCapabilityEntry,
    CoreCapabilityMatrix, CoreCapabilityScope, CoreLifecycleOperation, CoreOperation,
    EngineBinaryTarget, EngineCapabilityDecision, GeoOperation, HostCapabilityDecision,
    LogsOperation, OutboundOperation, ProfileOperation, ProviderOperation, ProxyControlOperation,
    VersionOperation,
};
use infiltrator_contract::core_identity::CoreCommandRequest;
use infiltrator_contract::surface::HostKind;
use infiltrator_contract::vpn::VpnGeneration;
use infiltrator_contract::vpn::{
    PreparedVpnConfiguration, VpnConfigurationApplyOutcome, VpnOwnerStopReceipt, VpnStartOutcome,
    VpnStopTarget,
};
use infiltrator_core::version_port::MihomoVersionPort;
use infiltrator_platform::android_bridge::{
    AndroidBridge, AndroidBridgeResult, CoreProcessStartReceipt, CoreProcessStopReceipt,
};
use infiltrator_ports::core_capability_provider::{
    CoreCapabilityProvider, CoreCapabilityProviderError,
};
use infiltrator_ports::core_lifecycle::CoreLifecyclePort;
use infiltrator_ports::engine_backend_factory::EngineBackendFactory;
use infiltrator_ports::engine_backend_factory::EngineBackendProvider;
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::secure_store::SecureStore;
use lurker_core_contract::active_profile_reference::ActiveProfileReferenceWriteAuthorization;
use lurker_core_contract::active_profile_reference::{
    ActiveProfileReferenceCasOutcome, ActiveProfileReferenceTarget,
};
use lurker_core_contract::capability::Capability;
use lurker_core_contract::core_process::CoreProcessStartContext;
use lurker_core_contract::error::ErrorCode;
use lurker_core_contract::error::Failure;
use lurker_core_contract::identity::CoreGeneration;
use lurker_core_contract::identity::{CoreVersion, EngineId};
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_contract::lifecycle::CoreLifecycleSnapshot;
use lurker_core_contract::outbound::{
    identity::{OutboundId, SnapshotRevision},
    probe::OutboundSelectionRequest,
};
use lurker_core_contract::profile_document_identity::ProfileFormatId;
use lurker_core_contract::profile_document_identity::ProfileId;
use lurker_core_contract::profile_protection::ProfileWriteAuthorization;
use lurker_core_ports::core_process::CoreReadiness;
use lurker_core_ports::error::PortError;
use mihomo_api::runtime_configuration::scope_resolver_from_owner;
use mihomo_config::manager::ConfigManager;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::time::timeout;

const INITIAL_DOCUMENT: &str = "mode: rule\n";
const SAVED_DOCUMENT: &str = "mode: direct\n";

#[path = "composition_tests/async_tests.rs"]
mod async_tests;
#[path = "composition_tests/singbox.rs"]
mod singbox;

struct TestCredentialStore;

#[async_trait]
impl SecureStore for TestCredentialStore {
    async fn get(&self, _namespace: &str, _key: &str) -> Result<Option<String>, PortError> {
        Ok(None)
    }

    async fn set(&self, _namespace: &str, _key: &str, _value: &str) -> Result<(), PortError> {
        Ok(())
    }

    async fn delete(&self, _namespace: &str, _key: &str) -> Result<(), PortError> {
        Ok(())
    }
}

struct TestHome(PathBuf);

impl Drop for TestHome {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn config_manager() -> (TestHome, Arc<ConfigManager<TestCredentialStore>>) {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock is after the Unix epoch")
        .as_nanos();
    let home = env::temp_dir().join(format!(
        "infiltrator-android-profile-owner-{}-{stamp}",
        process::id()
    ));
    fs::create_dir_all(&home).expect("temporary owner home is created");
    let manager = ConfigManager::with_home_and_store(home.clone(), TestCredentialStore)
        .expect("Mihomo config manager is constructed for the test home");
    (TestHome(home), Arc::new(manager))
}

async fn select_profile<S: SecureStore>(manager: &ConfigManager<S>, profile: &str) {
    let expected = ProfileStore::read_active_profile_reference(manager.engine_profile_store())
        .await
        .expect("read selection before test CAS");
    let target = ActiveProfileReferenceTarget::selected(
        ConfigManager::engine_id(manager),
        ProfileId::new(profile).expect("valid test profile ID"),
    );
    match ProfileStore::compare_and_set_active_profile_reference(
        manager.engine_profile_store(),
        expected,
        target,
        ActiveProfileReferenceWriteAuthorization::Unowned,
    )
    .await
    .expect("revisioned test selection CAS")
    {
        ActiveProfileReferenceCasOutcome::Committed { .. } => {}
        ActiveProfileReferenceCasOutcome::Stale { expected, observed } => {
            panic!("test selection unexpectedly stale: {expected:?} -> {observed:?}")
        }
    }
}

fn relative_entries(root: &Path) -> Vec<PathBuf> {
    fn collect(root: &Path, current: &Path, output: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(current).expect("test directory can be listed") {
            let entry = entry.expect("test directory entry can be read");
            let path = entry.path();
            output.push(path.strip_prefix(root).unwrap().to_path_buf());
            if path.is_dir() {
                collect(root, &path, output);
            }
        }
    }

    let mut output = Vec::new();
    collect(root, root, &mut output);
    output.sort();
    output
}

#[derive(Clone)]
struct TestBridge {
    observation: Arc<Mutex<AndroidCoreProcessObservation>>,
    launch_specs: Arc<Mutex<Vec<CoreLaunchSpec>>>,
    start_contexts: Arc<Mutex<Vec<CoreProcessStartContext>>>,
    data_dir_calls: Arc<AtomicUsize>,
}

impl TestBridge {
    fn new() -> Self {
        Self {
            observation: Arc::new(Mutex::new(AndroidCoreProcessObservation::NeverStarted)),
            launch_specs: Arc::new(Mutex::new(Vec::new())),
            start_contexts: Arc::new(Mutex::new(Vec::new())),
            data_dir_calls: Arc::new(AtomicUsize::new(0)),
        }
    }
}

#[async_trait]
impl AndroidBridge for TestBridge {
    fn process_role(&self) -> AndroidHostProcessRole {
        AndroidHostProcessRole::CoreOwner
    }

    async fn core_start(
        &self,
        launch_spec: &CoreLaunchSpec,
        context: &CoreProcessStartContext,
    ) -> AndroidBridgeResult<CoreProcessStartReceipt> {
        self.launch_specs
            .lock()
            .expect("test launch spec lock")
            .push(launch_spec.clone());
        self.start_contexts
            .lock()
            .expect("captured Core start contexts")
            .push(context.clone());
        *self.observation.lock().expect("test observation lock") =
            AndroidCoreProcessObservation::Running {
                owned: AndroidOwnedCoreProcess {
                    context: context.clone(),
                    child: AndroidChildProcessIdentity {
                        pid: 321,
                        start_time_ticks: 987_654,
                    },
                },
            };
        Ok(CoreProcessStartReceipt::Started)
    }

    async fn core_stop(&self) -> AndroidBridgeResult<CoreProcessStopReceipt> {
        *self.observation.lock().expect("test observation lock") =
            AndroidCoreProcessObservation::NeverStarted;
        Ok(CoreProcessStopReceipt::StoppedGracefully)
    }

    async fn core_process_observation(&self) -> AndroidBridgeResult<AndroidCoreProcessObservation> {
        Ok(self
            .observation
            .lock()
            .expect("test observation lock")
            .clone())
    }

    async fn credential_get(
        &self,
        _service: &str,
        _key: &str,
    ) -> AndroidBridgeResult<Option<String>> {
        Ok(None)
    }

    async fn credential_set(
        &self,
        _service: &str,
        _key: &str,
        _value: &str,
    ) -> AndroidBridgeResult<()> {
        Ok(())
    }

    async fn credential_delete(&self, _service: &str, _key: &str) -> AndroidBridgeResult<()> {
        Ok(())
    }

    fn data_dir(&self) -> Option<PathBuf> {
        self.data_dir_calls.fetch_add(1, Ordering::SeqCst);
        Some(PathBuf::from("/android-test-data"))
    }

    fn cache_dir(&self) -> Option<PathBuf> {
        None
    }

    async fn vpn_start(&self) -> AndroidBridgeResult<VpnStartOutcome> {
        Ok(VpnStartOutcome::PermissionRequired)
    }

    async fn vpn_apply_configuration(
        &self,
        _prepared: &PreparedVpnConfiguration,
    ) -> AndroidBridgeResult<VpnConfigurationApplyOutcome> {
        Ok(VpnConfigurationApplyOutcome::Rejected)
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

#[test]
fn android_mihomo_backend_provider_recreates_scoped_ports_without_hidden_state() {
    let (_home, store) = config_manager();
    let bridge = TestBridge::new();
    let bridge_probe = bridge.clone();
    let provider = Arc::new(
        super::AndroidMihomoBackendProvider::new(
            MihomoVersionPort::engine_identifier(),
            bridge,
            store,
            "http://127.0.0.1:9090",
            None,
            Arc::new(TestReadiness),
        )
        .expect("Mihomo engine admission succeeds before controller parsing"),
    );
    let provider_for_factory: Arc<dyn EngineBackendProvider> = provider;
    let factory = EngineBackendFactoryRegistry::new([provider_for_factory]).unwrap();
    let engine_id = MihomoVersionPort::engine_identifier();

    for _ in 0..2 {
        let backend = factory
            .backend_ports(&engine_id)
            .expect("the registered provider can repeatedly compose its ports");
        assert_eq!(backend.engine_id(), &engine_id);
        assert_eq!(backend.process().core_identity().unwrap().engine, engine_id);
        assert_eq!(backend.configuration().scope().engine_id, engine_id);
    }
    assert_eq!(bridge_probe.data_dir_calls.load(Ordering::SeqCst), 2);
}

#[test]
fn android_core_scope_resolver_rejects_a_stopped_owner() {
    let (_home, store) = config_manager();
    let provider = Arc::new(
        super::AndroidMihomoBackendProvider::new(
            MihomoVersionPort::engine_identifier(),
            TestBridge::new(),
            store,
            "http://127.0.0.1:9090",
            None,
            Arc::new(TestReadiness),
        )
        .expect("Mihomo engine admission succeeds"),
    );
    let provider_for_factory: Arc<dyn EngineBackendProvider> = provider;
    let factory = EngineBackendFactoryRegistry::new([provider_for_factory]).unwrap();
    let backend = factory
        .backend_ports(&MihomoVersionPort::engine_identifier())
        .expect("registered provider composes runtime ports");
    let application = CoreApplication::new_with_overview(
        backend.process(),
        backend.readiness(),
        Arc::new(UnavailableOverviewReader::new(PortError::unsupported(
            Capability::CoreLifecycle,
            "scope resolver test does not need overview polling",
        ))),
        infiltrator_composition::tokio_application_runtime().unwrap(),
    );

    let scope_resolver = scope_resolver_from_owner(Arc::new(application));
    let error = scope_resolver().expect_err("a stopped owner must not authorize controller access");

    assert!(
        matches!(error, PortError::Failed(message) if message.contains("owner scope is unavailable"))
    );
}

#[test]
fn android_capability_operations_follow_composed_controller_ports() {
    #[derive(Clone, Copy)]
    enum ControllerComposition {
        Available,
        Unavailable,
    }

    let unavailable = super::AndroidControllerAdapters::new("invalid-controller-endpoint", None);
    assert!(matches!(
        &unavailable,
        super::AndroidControllerAdapters::Unavailable { .. }
    ));
    let available = super::AndroidControllerAdapters::new("http://127.0.0.1:9090", None);
    assert!(matches!(
        &available,
        super::AndroidControllerAdapters::Available { .. }
    ));
    let cases = [
        (unavailable, ControllerComposition::Unavailable),
        (available, ControllerComposition::Available),
    ];
    let engine_id = MihomoVersionPort::engine_identifier();
    let version = CoreVersion::new("v1.20.3").expect("test version");
    let lifecycle_operations = [
        CoreOperation::Lifecycle(CoreLifecycleOperation::Start),
        CoreOperation::Lifecycle(CoreLifecycleOperation::Stop),
        CoreOperation::Lifecycle(CoreLifecycleOperation::Restart),
    ];
    let runtime_operations = [
        CoreOperation::Outbound(OutboundOperation::List),
        CoreOperation::Outbound(OutboundOperation::Select),
        CoreOperation::Outbound(OutboundOperation::Probe),
        CoreOperation::Profile(ProfileOperation::Validate),
        CoreOperation::Profile(ProfileOperation::Apply),
        CoreOperation::Connection(ConnectionOperation::Close),
        CoreOperation::Logs(LogsOperation::SetLevel),
        CoreOperation::Logs(LogsOperation::Stream),
        CoreOperation::Geo(GeoOperation::UpdateDatabases),
        CoreOperation::Provider(ProviderOperation::ListRule),
        CoreOperation::Provider(ProviderOperation::UpdateRule),
        CoreOperation::ProxyControl(ProxyControlOperation::SetRoutingIntent),
        CoreOperation::ProxyControl(ProxyControlOperation::GetRoutingIntent),
    ];

    for (controller_adapters, expected_composition) in cases {
        let matrix = controller_adapters
            .capability_provider()
            .matrix_for(&engine_id, Some(&version))
            .expect("Android Mihomo provider authors its matrix");
        let expected_operations = match expected_composition {
            ControllerComposition::Available => lifecycle_operations
                .iter()
                .chain(runtime_operations.iter())
                .copied()
                .collect::<Vec<_>>(),
            ControllerComposition::Unavailable => lifecycle_operations.to_vec(),
        };

        for operation in CoreOperation::all().iter().copied() {
            let entry = matrix
                .entries
                .iter()
                .find(|entry| entry.operation == operation)
                .expect("the provider registers every Core operation");
            match expected_operations.contains(&operation) {
                true => {
                    assert_eq!(entry.adapter, AdapterCapabilityDecision::Supported);
                }
                false => {
                    assert!(matches!(
                        &entry.adapter,
                        AdapterCapabilityDecision::AdapterMissing { .. }
                    ));
                }
            }
        }

        let connection_list = matrix
            .entries
            .iter()
            .find(|entry| entry.operation == CoreOperation::Connection(ConnectionOperation::List))
            .expect("connection list remains in the full operation catalogue");
        assert!(matches!(
            &connection_list.engine,
            EngineCapabilityDecision::Unsupported { .. }
        ));
        assert!(matches!(
            &connection_list.adapter,
            AdapterCapabilityDecision::AdapterMissing { .. }
        ));

        for operation in [
            CoreOperation::Profile(ProfileOperation::Validate),
            CoreOperation::Profile(ProfileOperation::Apply),
        ] {
            assert!(matches!(
                &matrix
                    .entries
                    .iter()
                    .find(|entry| entry.operation == operation)
                    .expect("profile operation is registered")
                    .engine,
                EngineCapabilityDecision::Unsupported { .. }
            ));
        }

        for operation in [
            CoreOperation::Version(VersionOperation::Inspect),
            CoreOperation::Version(VersionOperation::Rollback),
        ] {
            let entry = matrix
                .entries
                .iter()
                .find(|entry| entry.operation == operation)
                .expect("version operation is registered");
            assert!(matches!(
                &entry.adapter,
                AdapterCapabilityDecision::AdapterMissing { .. }
            ));
        }
    }
}

#[test]
fn unknown_android_engine_is_unsupported_before_bridge_or_controller_side_effects() {
    let (home, store) = config_manager();
    let files_before = relative_entries(&home.0);
    let bridge = TestBridge::new();
    let bridge_probe = bridge.clone();
    let result = super::core_application_for_engine(
        EngineId::new("future-engine").unwrap(),
        bridge,
        "invalid-controller-endpoint".to_owned(),
        None,
        AndroidCoreComposition::new(store),
        Arc::new(TestReadiness),
        super::AndroidCapabilitySource::Provided(Arc::new(TestCapabilityProvider)),
    );

    assert!(matches!(
        result,
        Err(PortError::Unsupported {
            capability: Capability::CoreLifecycle,
            ..
        })
    ));
    assert_eq!(bridge_probe.data_dir_calls.load(Ordering::SeqCst), 0);
    assert!(bridge_probe.launch_specs.lock().unwrap().is_empty());
    assert_eq!(
        *bridge_probe.observation.lock().unwrap(),
        AndroidCoreProcessObservation::NeverStarted
    );
    assert_eq!(relative_entries(&home.0), files_before);
}

struct TestReadiness;

#[async_trait]
impl CoreReadiness for TestReadiness {
    async fn probe(&self) -> Result<String, PortError> {
        Ok("test-controller-ready".to_owned())
    }

    async fn active_core_version(&self) -> Result<Option<CoreVersion>, PortError> {
        Ok(Some(
            CoreVersion::new("v1.20.3").expect("test version is a typed value"),
        ))
    }
}

struct NoVersionReadiness;

#[async_trait]
impl CoreReadiness for NoVersionReadiness {
    async fn probe(&self) -> Result<String, PortError> {
        Ok("test-controller-ready".to_owned())
    }

    async fn active_core_version(&self) -> Result<Option<CoreVersion>, PortError> {
        Ok(None)
    }
}

struct TestCapabilityProvider;
struct TestLifecycleCapabilityProvider;

fn test_capability_matrix(
    engine_id: &EngineId,
    core_version: Option<&CoreVersion>,
    operations: &[CoreOperation],
) -> Result<CoreCapabilityMatrix, CoreCapabilityProviderError> {
    let entries = operations
        .iter()
        .copied()
        .map(|operation| {
            CoreCapabilityEntry::new(
                operation,
                EngineCapabilityDecision::Supported,
                HostCapabilityDecision::Supported,
                AdapterCapabilityDecision::Supported,
            )
        })
        .collect();
    let binary_target = EngineBinaryTarget::current();
    CoreCapabilityMatrix::new(
        CoreCapabilityScope::new_for_target(
            engine_id.clone(),
            core_version.cloned(),
            binary_target.host_kind().unwrap_or(HostKind::Desktop),
            binary_target,
            AdapterRevision::new(99),
        ),
        entries,
    )
    .map_err(|error| CoreCapabilityProviderError::MatrixUnavailable {
        reason: error.to_string(),
    })
}

impl CoreCapabilityProvider for TestCapabilityProvider {
    fn matrix_for(
        &self,
        engine_id: &EngineId,
        core_version: Option<&CoreVersion>,
    ) -> Result<CoreCapabilityMatrix, CoreCapabilityProviderError> {
        test_capability_matrix(
            engine_id,
            core_version,
            &[
                CoreOperation::Lifecycle(CoreLifecycleOperation::Start),
                CoreOperation::Profile(ProfileOperation::Apply),
                CoreOperation::Outbound(OutboundOperation::List),
                CoreOperation::Outbound(OutboundOperation::Select),
            ],
        )
    }
}

impl CoreCapabilityProvider for TestLifecycleCapabilityProvider {
    fn matrix_for(
        &self,
        engine_id: &EngineId,
        core_version: Option<&CoreVersion>,
    ) -> Result<CoreCapabilityMatrix, CoreCapabilityProviderError> {
        test_capability_matrix(
            engine_id,
            core_version,
            &[CoreOperation::Lifecycle(CoreLifecycleOperation::Start)],
        )
    }
}

async fn run_scoped(application: &CoreApplication, intent: CommandIntent) -> CommandResult {
    let scope = application
        .snapshot()
        .scope()
        .expect("the owner publishes its current core scope");
    let request = AndroidProductCommandRequest {
        scope,
        command: AndroidProductCommand::Application(Box::new(intent)),
    };
    let wire = serde_json::to_string(&request).expect("typed owner request serializes");
    let request = serde_json::from_str::<AndroidProductCommandRequest>(&wire)
        .expect("typed owner request deserializes");
    let AndroidProductCommand::Application(intent) = request.command else {
        panic!("this test submits only application commands");
    };
    application
        .execute_scoped(CoreCommandRequest {
            scope: request.scope,
            intent: *intent,
        })
        .await
        .result
}

fn produced(result: CommandResult) -> CommandOutput {
    match result {
        CommandResult::Produced { output, .. } => *output,
        other => panic!("owner command should produce a typed profile result: {other:?}"),
    }
}

fn rejected(result: CommandResult, code: ErrorCode) {
    match result {
        CommandResult::Rejected { failure, .. } => assert_eq!(failure.code, code),
        other => panic!("owner must reject the unsafe profile write: {other:?}"),
    }
}

async fn assert_local_load_succeeds_and_runtime_apply_fails_closed(
    readiness: Arc<dyn CoreReadiness>,
) {
    let (_home, store) = config_manager();
    store
        .save_inactive_profile("main", INITIAL_DOCUMENT)
        .await
        .unwrap();
    let owner = super::core_application_with_readiness_and_provider(
        TestBridge::new(),
        "invalid-controller-endpoint".to_owned(),
        None,
        AndroidCoreComposition::new(Arc::clone(&store)),
        readiness,
        Arc::new(TestLifecycleCapabilityProvider),
    )
    .expect("Android owner runtime is constructible");
    assert!(matches!(
        run_scoped(&owner, CommandIntent::StartCore).await,
        CommandResult::Completed { .. }
    ));

    let loaded = produced(
        run_scoped(
            &owner,
            CommandIntent::LoadProfileDocument {
                profile: Some("main".to_owned()),
            },
        )
        .await,
    );
    let document = match loaded {
        CommandOutput::ProfileDocumentLoaded(document) => *document,
        other => panic!("local profile load must return its composed document: {other:?}"),
    };
    assert_eq!(document.content, INITIAL_DOCUMENT);

    let options = produced(
        run_scoped(
            &owner,
            CommandIntent::LoadProfileOptions {
                profile: Some("main".to_owned()),
            },
        )
        .await,
    );
    assert!(matches!(options, CommandOutput::ProfileOptionsLoaded(_)));

    rejected(
        run_scoped(
            &owner,
            CommandIntent::SaveProfileDocument {
                workspace_identity: document.workspace_identity,
                content: SAVED_DOCUMENT.to_owned(),
                authorization: ProfileWriteAuthorization::RequireUnlock,
            },
        )
        .await,
        ErrorCode::Unsupported,
    );
    assert_eq!(store.load("main").await.unwrap(), INITIAL_DOCUMENT);
}
