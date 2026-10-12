//! Tests for the Desktop sing-box runtime composition.

use super::*;
use async_trait::async_trait;
use infiltrator_composition::active_profile_backend::preflight_active_profile_reference;
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReferenceCasOutcome, ActiveProfileReferenceState, ActiveProfileReferenceTarget,
    ActiveProfileReferenceWriteAuthorization,
};
use infiltrator_contract::core_capabilities::CoreLifecycleOperation;
use infiltrator_contract::core_capabilities::{CoreCapabilityMatrix, EngineBinaryTarget};
use infiltrator_contract::core_identity::CoreVersion;
use infiltrator_contract::profile_document_identity::ProfileId;
use infiltrator_ports::core_binary_selection::CoreBinarySelectionSource;
use infiltrator_ports::core_capability_provider::CoreCapabilityProviderError;
use infiltrator_ports::error::PortError;
use mihomo_config::manager::{ConfigManager, ProfileDocumentExtension};
use mihomo_platform::defaults::DefaultCredentialStore;
use reqwest::Url;
use singbox_adapter::capability_provider::SingBoxCoreCapabilityProvider;
use singbox_adapter::command::{
    NativeConfigCheck, SingBoxChildPoll, SingBoxChildProcess, SingBoxCommandPort,
};
use singbox_adapter::controller_configuration::SingBoxControllerConfigurationError;
use singbox_adapter::http::{
    SingBoxClashHttpPort, SingBoxClashMethod, SingBoxClashResponse, SingBoxControllerAuthorization,
};
use singbox_adapter::profile_document_codec::SingBoxProfileDocumentCodec;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;
use tempfile::TempDir;

struct SelectedSingBoxFixture {
    _home: TempDir,
    reference_store: Arc<dyn ActiveProfileReferenceStore>,
    profile_store: Arc<dyn ProfileStore>,
    config_path: PathBuf,
    preflight: ActiveProfileSelectionPreflight,
}

impl SelectedSingBoxFixture {
    async fn new(profile: &str) -> Self {
        let home = TempDir::new().expect("create desktop sing-box fixture home");
        let manager = Arc::new(
            ConfigManager::with_profile_codec_and_store_at_root(
                home.path().to_path_buf(),
                home.path().join("configs"),
                DefaultCredentialStore::default(),
                Arc::new(SingBoxProfileDocumentCodec),
                ProfileDocumentExtension::Json,
            )
            .expect("create sing-box JSON profile manager"),
        );
        manager
            .initialize_profile_namespace()
            .await
            .expect("initialize sing-box profile namespace");
        let profile_id = ProfileId::new("primary").expect("valid profile id");
        let profile_store: Arc<dyn ProfileStore> = manager.clone();
        profile_store
            .save_inactive_profile(profile_id.as_str(), profile)
            .await
            .expect("save sing-box fixture profile");
        let initial = profile_store
            .read_active_profile_reference()
            .await
            .expect("read initial profile reference");
        let committed = profile_store
            .compare_and_set_active_profile_reference(
                initial,
                ActiveProfileReferenceTarget::selected(engine_id(), profile_id.clone()),
                ActiveProfileReferenceWriteAuthorization::Unowned,
            )
            .await
            .expect("select sing-box fixture profile");
        assert!(matches!(
            committed,
            ActiveProfileReferenceCasOutcome::Committed {
                current: ActiveProfileReferenceState::Selected(_),
                ..
            }
        ));
        let reference_store: Arc<dyn ActiveProfileReferenceStore> = manager.clone();
        let capabilities: Arc<dyn CoreCapabilityProvider> =
            Arc::new(SingBoxCoreCapabilityProvider::new(
                HostKind::Desktop,
                [
                    CoreOperation::Lifecycle(CoreLifecycleOperation::Start),
                    CoreOperation::Lifecycle(CoreLifecycleOperation::Stop),
                ],
            ));
        let preflight = preflight_active_profile_reference(
            Arc::clone(&reference_store),
            [Arc::clone(&profile_store)],
            capabilities,
        )
        .await
        .expect("preflight selected sing-box profile");
        let config_path = manager
            .existing_profile_document_path(profile_id.as_str())
            .await
            .expect("resolve sing-box JSON profile path");
        Self {
            _home: home,
            reference_store,
            profile_store,
            config_path,
            preflight,
        }
    }
}

#[derive(Default)]
struct FakeCommandCounts {
    checks: AtomicUsize,
    spawns: AtomicUsize,
    running: AtomicBool,
}

struct FakeSingBoxCommand(Arc<FakeCommandCounts>);

struct FakeChild(Arc<FakeCommandCounts>);

#[async_trait]
impl SingBoxChildProcess for FakeChild {
    fn pid(&self) -> u32 {
        49_001
    }

    async fn poll(&mut self) -> Result<SingBoxChildPoll, PortError> {
        Ok(match self.0.running.load(Ordering::SeqCst) {
            true => SingBoxChildPoll::Running,
            false => SingBoxChildPoll::Exited,
        })
    }

    async fn terminate(&mut self) -> Result<(), PortError> {
        self.0.running.store(false, Ordering::SeqCst);
        Ok(())
    }
}

#[async_trait]
impl SingBoxCommandPort for FakeSingBoxCommand {
    async fn check_config(
        &self,
        _binary_path: &Path,
        _config_path: &Path,
    ) -> Result<NativeConfigCheck, PortError> {
        self.0.checks.fetch_add(1, Ordering::SeqCst);
        Ok(NativeConfigCheck::Accepted)
    }

    async fn spawn_core(
        &self,
        _binary_path: &Path,
        _config_path: &Path,
    ) -> Result<Box<dyn SingBoxChildProcess>, PortError> {
        self.0.spawns.fetch_add(1, Ordering::SeqCst);
        self.0.running.store(true, Ordering::SeqCst);
        Ok(Box::new(FakeChild(Arc::clone(&self.0))))
    }
}

struct FakeSingBoxHttp;

#[async_trait]
impl SingBoxClashHttpPort for FakeSingBoxHttp {
    async fn request(
        &self,
        url: &Url,
        method: SingBoxClashMethod,
        _authorization: &SingBoxControllerAuthorization,
        _body: Option<Vec<u8>>,
        _timeout: Duration,
    ) -> Result<SingBoxClashResponse, PortError> {
        if method != SingBoxClashMethod::Get || url.path() != "/version" {
            return Err(PortError::Failed(
                "desktop sing-box fake received an unexpected request".to_owned(),
            ));
        }
        Ok(SingBoxClashResponse {
            status: 200,
            body: br#"{"version":"sing-box 1.14.3"}"#.to_vec(),
        })
    }
}

fn fake_ports(
    counts: Arc<FakeCommandCounts>,
) -> (Arc<dyn SingBoxCommandPort>, Arc<dyn SingBoxClashHttpPort>) {
    (
        Arc::new(FakeSingBoxCommand(counts)),
        Arc::new(FakeSingBoxHttp),
    )
}

fn verified_selection() -> CoreBinarySelection {
    let engine_id = engine_id();
    let version = CoreVersion::new("v1.14.3").expect("locked sing-box version is valid");
    CoreBinarySelection {
        engine_id: engine_id.clone(),
        version: version.clone(),
        binary_path: PathBuf::from("/managed/sing-box/1.14.3/sing-box"),
        related_asset_candidates: Vec::new(),
        source: CoreBinarySelectionSource::ManagedVersion,
        verification: CoreArtifactVerification::Verified {
            engine_id,
            version: version.raw().to_owned(),
        },
    }
}

#[test]
fn selection_requires_managed_source_and_matching_verification() {
    let engine_id = engine_id();
    let selection = verified_selection();
    validate_selection(&selection, &engine_id).expect("verified managed selection is accepted");

    let mut wrong_version = selection.clone();
    wrong_version.version = CoreVersion::new("v1.14.4").expect("valid test version");
    assert!(matches!(
        validate_selection(&wrong_version, &engine_id),
        Err(PortError::Unsupported {
            capability: Capability::CoreLifecycle,
            ..
        })
    ));

    let mut bundled = selection;
    bundled.source = CoreBinarySelectionSource::BundledPackage {
        content_sha256: "a".repeat(64),
    };
    assert!(matches!(
        validate_selection(&bundled, &engine_id),
        Err(PortError::Unsupported {
            capability: Capability::CoreLifecycle,
            ..
        })
    ));
}

#[test]
fn selected_version_must_authorize_core_lifecycle_for_the_current_target() {
    let engine_id = engine_id();
    let provider: Arc<dyn CoreCapabilityProvider> = Arc::new(SingBoxCoreCapabilityProvider::new(
        HostKind::Desktop,
        [
            CoreOperation::Lifecycle(CoreLifecycleOperation::Start),
            CoreOperation::Lifecycle(CoreLifecycleOperation::Stop),
        ],
    ));
    validate_version_capability(&provider, &verified_selection(), &engine_id)
        .expect("locked target/version authorizes Core lifecycle");
}

struct WrongTargetCapabilityProvider(SingBoxCoreCapabilityProvider);

impl CoreCapabilityProvider for WrongTargetCapabilityProvider {
    fn matrix_for(
        &self,
        engine_id: &EngineId,
        version: Option<&CoreVersion>,
    ) -> Result<CoreCapabilityMatrix, CoreCapabilityProviderError> {
        let mut matrix = self.0.matrix_for(engine_id, version)?;
        matrix.scope.binary_target = match EngineBinaryTarget::current() {
            EngineBinaryTarget::X86_64UnknownLinuxGnu => EngineBinaryTarget::Aarch64UnknownLinuxGnu,
            _ => EngineBinaryTarget::X86_64UnknownLinuxGnu,
        };
        Ok(matrix)
    }
}

#[test]
fn target_mismatched_capability_matrix_fails_before_runtime_construction() {
    let engine_id = engine_id();
    let provider: Arc<dyn CoreCapabilityProvider> = Arc::new(WrongTargetCapabilityProvider(
        SingBoxCoreCapabilityProvider::new(
            HostKind::Desktop,
            [CoreOperation::Lifecycle(CoreLifecycleOperation::Start)],
        ),
    ));
    assert!(matches!(
        validate_version_capability(&provider, &verified_selection(), &engine_id),
        Err(PortError::Rejected(Failure {
            code: ErrorCode::InvalidState,
            ..
        }))
    ));
}

#[test]
fn surface_advertises_only_the_connected_lifecycle_capability() {
    let capabilities = singbox_surface_capabilities();
    assert!(capabilities.supports(Capability::CoreLifecycle));
    assert!(!capabilities.supports(Capability::Profiles));
    assert!(!capabilities.supports(Capability::RoutingIntent));
    assert!(!capabilities.supports(Capability::Connections));
    assert!(!capabilities.supports(Capability::Logs));
}

#[tokio::test]
async fn missing_or_public_clash_api_fails_before_native_process_commands() {
    let cases = [
        (
            r#"{"log":{"level":"error"},"outbounds":[]}"#,
            SingBoxControllerConfigurationError::ClashApiDisabled,
        ),
        (
            r#"{"log":{"level":"error"},"outbounds":[],"experimental":{"clash_api":{"external_controller":"0.0.0.0:9090"}}}"#,
            SingBoxControllerConfigurationError::ExternalControllerNotLoopback,
        ),
    ];

    for (profile, expected) in cases {
        let fixture = SelectedSingBoxFixture::new(profile).await;
        let counts = Arc::new(FakeCommandCounts::default());
        let (command, http) = fake_ports(Arc::clone(&counts));
        let error = match SingBoxRuntime::prepare_with_ports(
            verified_selection(),
            Arc::clone(&fixture.reference_store),
            Arc::clone(&fixture.profile_store),
            fixture.config_path.clone(),
            fixture.preflight,
            command,
            http,
        )
        .await
        {
            Ok(_) => panic!("invalid sing-box controller profile must fail preflight"),
            Err(error) => error,
        };

        assert_eq!(
            error
                .downcast_ref::<SingBoxControllerConfigurationError>()
                .expect("typed controller preflight error"),
            &expected
        );
        assert_eq!(counts.checks.load(Ordering::SeqCst), 0);
        assert_eq!(counts.spawns.load(Ordering::SeqCst), 0);
    }
}

#[tokio::test]
async fn desktop_runtime_starts_only_on_core_lifecycle_intent_and_reads_real_state() {
    let fixture = SelectedSingBoxFixture::new(
            r#"{"log":{"level":"error"},"outbounds":[{"type":"direct","tag":"direct"}],"experimental":{"clash_api":{"external_controller":"127.0.0.1:9090"}}}"#,
        )
        .await;
    let counts = Arc::new(FakeCommandCounts::default());
    let (command, http) = fake_ports(Arc::clone(&counts));
    let runtime = SingBoxRuntime::prepare_with_ports(
        verified_selection(),
        Arc::clone(&fixture.reference_store),
        Arc::clone(&fixture.profile_store),
        fixture.config_path.clone(),
        fixture.preflight,
        command,
        http,
    )
    .await
    .expect("compose one offline sing-box Core owner");
    let application = runtime.application();

    assert_eq!(runtime.core_lifecycle(), CoreLifecycle::Stopped);
    assert_eq!(counts.checks.load(Ordering::SeqCst), 0);
    assert_eq!(counts.spawns.load(Ordering::SeqCst), 0);
    CoreLifecyclePort::start(application.as_ref())
        .await
        .expect("explicit lifecycle intent starts sing-box");
    assert_eq!(runtime.core_lifecycle(), CoreLifecycle::Running);
    assert_eq!(counts.checks.load(Ordering::SeqCst), 1);
    assert_eq!(counts.spawns.load(Ordering::SeqCst), 1);

    ManagedRuntime::shutdown(&runtime)
        .await
        .expect("explicit stop intent terminates sing-box");
    assert_eq!(runtime.core_lifecycle(), CoreLifecycle::Stopped);
    assert!(!counts.running.load(Ordering::SeqCst));
    application
        .close()
        .await
        .expect("close and release the exact Core owner");
}
