use super::AndroidSingBoxProfileLaunchSpecProvider;
use crate::runtime::CoreLaunchSpecSource;
use async_trait::async_trait;
use infiltrator_contract::android_control::{
    AndroidCoreBinaryTarget, CoreLaunchArguments, CoreLaunchBinary,
};
use infiltrator_contract::core_capabilities::EngineBinaryTarget;
use infiltrator_contract::core_identity::{CoreVersion, EngineId};
use infiltrator_contract::error::ErrorCode;
use infiltrator_contract::profile_document_identity::ProfileId;
use infiltrator_contract::version::CoreArtifactVerification;
use infiltrator_ports::core_binary_selection::{
    CoreBinarySelection, CoreBinarySelectionPolicy, CoreBinarySelectionPort,
    CoreBinarySelectionRequest, CoreBinarySelectionSource,
};
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::secure_store::SecureStore;
use mihomo_config::manager::{ConfigManager, ProfileDocumentExtension};
use singbox_config::profile_document_codec::SingBoxProfileDocumentCodec;
use singbox_process::command::{NativeConfigCheck, SingBoxChildProcess, SingBoxCommandPort};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

const VALID_PROFILE: &[u8] = br#"{
  "experimental": { "clash_api": { "external_controller": "127.0.0.1:9090" } },
  "inbounds": [],
  "outbounds": []
}"#;

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

struct FixedBinarySelection(CoreBinarySelection);

#[async_trait]
impl CoreBinarySelectionPort for FixedBinarySelection {
    fn engine_id(&self) -> EngineId {
        self.0.engine_id.clone()
    }

    async fn select(
        &self,
        request: CoreBinarySelectionRequest,
    ) -> Result<CoreBinarySelection, PortError> {
        if request.engine_id != self.0.engine_id
            || request.policy != CoreBinarySelectionPolicy::ManagedOnly
        {
            return Err(PortError::Failed(
                "test selection received a mismatched engine or policy".to_owned(),
            ));
        }
        Ok(self.0.clone())
    }
}

struct RecordingSingBoxCommand {
    check_result: NativeConfigCheck,
    checked_paths: Mutex<Vec<(PathBuf, PathBuf)>>,
}

impl RecordingSingBoxCommand {
    fn new(check_result: NativeConfigCheck) -> Self {
        Self {
            check_result,
            checked_paths: Mutex::new(Vec::new()),
        }
    }

    fn checked_paths(&self) -> Vec<(PathBuf, PathBuf)> {
        self.checked_paths.lock().expect("check path lock").clone()
    }
}

#[async_trait]
impl SingBoxCommandPort for RecordingSingBoxCommand {
    async fn check_config(
        &self,
        binary_path: &Path,
        config_path: &Path,
    ) -> Result<NativeConfigCheck, PortError> {
        self.checked_paths
            .lock()
            .expect("check path lock")
            .push((binary_path.to_path_buf(), config_path.to_path_buf()));
        Ok(self.check_result)
    }

    async fn spawn_core(
        &self,
        _binary_path: &Path,
        _config_path: &Path,
    ) -> Result<Box<dyn SingBoxChildProcess>, PortError> {
        Err(PortError::unsupported(
            infiltrator_contract::capability::Capability::CoreLifecycle,
            "launch-spec tests never spawn a Core process",
        ))
    }
}

struct Fixture {
    _home: TempDir,
    working_directory: PathBuf,
    profile_store: Arc<ConfigManager<TestCredentialStore>>,
    profile_document: infiltrator_contract::profile_document_identity::ProfileDocument,
    configuration_path: PathBuf,
    binary_selection: CoreBinarySelection,
}

impl Fixture {
    async fn new(profile_bytes: &[u8]) -> Self {
        let home = TempDir::new().expect("temporary Android app data directory");
        let working_directory = home.path().to_path_buf();
        let profile_store = Arc::new(
            ConfigManager::with_profile_codec_and_store(
                working_directory.clone(),
                None,
                TestCredentialStore,
                Arc::new(SingBoxProfileDocumentCodec),
                ProfileDocumentExtension::Json,
            )
            .expect("sing-box profile store"),
        );
        profile_store
            .initialize_profile_namespace()
            .await
            .expect("initialize sing-box namespace");
        ProfileStore::save_inactive_profile(
            profile_store.as_ref(),
            "main",
            std::str::from_utf8(profile_bytes).expect("profile JSON is UTF-8"),
        )
        .await
        .expect("save sing-box profile");
        let profile_id = ProfileId::new("main").expect("profile ID");
        let workspace = ProfileStore::load_workspace(profile_store.as_ref(), &profile_id)
            .await
            .expect("load persisted profile");
        let configuration_path = profile_store
            .existing_profile_document_path(profile_id.as_str())
            .await
            .expect("profile path");

        let engine_id = singbox_api::engine_id();
        let version = CoreVersion::new("v1.14.3").expect("sing-box version");
        let binary_target = EngineBinaryTarget::Aarch64LinuxAndroid;
        let target_triple = "aarch64-linux-android";
        let binary_path = working_directory
            .join("engines/sing-box/targets")
            .join(target_triple)
            .join("versions")
            .join(version.raw())
            .join("sing-box");
        tokio::fs::create_dir_all(binary_path.parent().expect("binary parent"))
            .await
            .expect("create managed version directory");
        tokio::fs::write(&binary_path, b"test binary")
            .await
            .expect("write managed binary fixture");
        let binary_selection = CoreBinarySelection {
            engine_id: engine_id.clone(),
            binary_target,
            version: version.clone(),
            binary_path,
            related_asset_candidates: Vec::new(),
            source: CoreBinarySelectionSource::ManagedVersion,
            verification: CoreArtifactVerification::Verified {
                engine_id,
                version: version.raw().to_owned(),
            },
        };

        Self {
            _home: home,
            working_directory,
            profile_store,
            profile_document: workspace.document,
            configuration_path,
            binary_selection,
        }
    }

    fn provider(
        &self,
        command: Arc<dyn SingBoxCommandPort>,
    ) -> AndroidSingBoxProfileLaunchSpecProvider<TestCredentialStore> {
        self.provider_with_selection(self.binary_selection.clone(), command)
    }

    fn provider_with_selection(
        &self,
        selection: CoreBinarySelection,
        command: Arc<dyn SingBoxCommandPort>,
    ) -> AndroidSingBoxProfileLaunchSpecProvider<TestCredentialStore> {
        AndroidSingBoxProfileLaunchSpecProvider::new(
            Arc::clone(&self.profile_store),
            self.profile_document.clone(),
            Arc::new(FixedBinarySelection(selection)),
            self.working_directory.clone(),
            EngineBinaryTarget::Aarch64LinuxAndroid,
            command,
        )
    }
}

#[tokio::test]
async fn launch_spec_binds_exact_managed_binary_target_profile_and_native_check() {
    let fixture = Fixture::new(VALID_PROFILE).await;
    let command = Arc::new(RecordingSingBoxCommand::new(NativeConfigCheck::Accepted));
    let spec = fixture
        .provider(command.clone())
        .resolve()
        .await
        .expect("launch spec follows native validation");

    assert_eq!(
        spec.configuration_identity().engine_id,
        singbox_api::engine_id()
    );
    assert_eq!(spec.configuration_identity().profile_id.as_str(), "main");
    assert_eq!(
        spec.configuration_identity().document_revision,
        fixture.profile_document.identity.revision
    );
    assert_eq!(
        spec.working_directory().as_path(),
        fixture.working_directory
    );
    assert_eq!(
        spec.launch_arguments().configuration_path().as_str(),
        fixture.configuration_path.to_string_lossy()
    );
    assert!(matches!(
        spec.binary(),
        CoreLaunchBinary::SingBoxManaged {
            version,
            target: AndroidCoreBinaryTarget::Arm64V8a,
        } if version.raw() == "v1.14.3"
    ));
    assert!(matches!(
        spec.launch_arguments(),
        CoreLaunchArguments::SingBox { .. }
    ));
    assert_eq!(command.checked_paths().len(), 1);
    assert_eq!(command.checked_paths()[0].1, fixture.configuration_path);
}

#[tokio::test]
async fn stale_profile_revision_is_rejected_before_native_check() {
    let fixture = Fixture::new(VALID_PROFILE).await;
    let command = Arc::new(RecordingSingBoxCommand::new(NativeConfigCheck::Accepted));
    let provider = fixture.provider(command.clone());
    ProfileStore::save_inactive_profile(
        fixture.profile_store.as_ref(),
        "main",
        r#"{"outbounds":[{"type":"direct","tag":"changed"}]}"#,
    )
    .await
    .expect("replace stale profile");

    let failure = provider
        .resolve()
        .await
        .expect_err("stale preflight profile is rejected");
    assert!(matches!(
        failure,
        PortError::Rejected(ref failure) if failure.code == ErrorCode::NotReady
    ));
    assert!(command.checked_paths().is_empty());
}

#[tokio::test]
async fn native_profile_rejection_never_produces_a_launch_spec() {
    let fixture = Fixture::new(VALID_PROFILE).await;
    let command = Arc::new(RecordingSingBoxCommand::new(NativeConfigCheck::Rejected));

    let failure = fixture
        .provider(command.clone())
        .resolve()
        .await
        .expect_err("native sing-box rejection blocks launch");
    assert!(matches!(
        failure,
        PortError::Rejected(ref failure) if failure.code == ErrorCode::Configuration
    ));
    assert_eq!(command.checked_paths().len(), 1);
}

#[tokio::test]
async fn foreign_binary_target_is_rejected_before_native_check() {
    let fixture = Fixture::new(VALID_PROFILE).await;
    let command = Arc::new(RecordingSingBoxCommand::new(NativeConfigCheck::Accepted));
    let provider = AndroidSingBoxProfileLaunchSpecProvider::new(
        Arc::clone(&fixture.profile_store),
        fixture.profile_document.clone(),
        Arc::new(FixedBinarySelection(fixture.binary_selection.clone())),
        fixture.working_directory.clone(),
        EngineBinaryTarget::X86_64LinuxAndroid,
        command.clone(),
    );

    assert!(provider.resolve().await.is_err());
    assert!(command.checked_paths().is_empty());
}

#[cfg(unix)]
#[tokio::test]
async fn managed_binary_symlink_cannot_escape_its_version_directory() {
    use std::os::unix::fs::symlink;

    let fixture = Fixture::new(VALID_PROFILE).await;
    let selection = fixture.binary_selection.clone();
    let binary_path = selection.binary_path.clone();
    let outside_binary = fixture.working_directory.join("unmanaged-sing-box");
    tokio::fs::write(&outside_binary, b"unmanaged test binary")
        .await
        .expect("write out-of-directory binary");
    tokio::fs::remove_file(&binary_path)
        .await
        .expect("remove managed binary fixture");
    symlink(&outside_binary, &binary_path).expect("link managed path to outside binary");
    let command = Arc::new(RecordingSingBoxCommand::new(NativeConfigCheck::Accepted));

    assert!(
        fixture
            .provider_with_selection(selection, command.clone())
            .resolve()
            .await
            .is_err()
    );
    assert!(command.checked_paths().is_empty());
}
