//! test-intent: behavior

use infiltrator_contract::capability::Capability;
use std::str::from_utf8;
use tokio::fs;
use crate::engine_backend_factory::SingBoxEngineBackendProvider;
use async_trait::async_trait;
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReferenceCasOutcome, ActiveProfileReferenceState, ActiveProfileReferenceTarget,
    ActiveProfileReferenceWriteAuthorization,
};
use infiltrator_contract::core_identity::{CoreIdentity, CoreInstanceId, EngineId};
use infiltrator_contract::profile_document_identity::ProfileId;
use infiltrator_contract::session::SessionToken;
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_domain::apply::ApplyStrategy;
use infiltrator_ports::core_lifecycle::CoreLifecyclePort;
use infiltrator_ports::engine_backend_factory::EngineBackendProvider;
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_configuration_transaction::{
    ProfileConfigurationCommitEffect, ProfileConfigurationTransactionOutcome,
    ProfileConfigurationTransactionPort, ProfileConfigurationTransactionRequest,
};
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::profile_workspace::{ProfileWorkspacePurpose, ProfileWorkspaceUpdate};
use infiltrator_ports::secure_store::SecureStore;
use infiltrator_profile_store::store::ProfileDocumentExtension;
use mihomo_config::manager::ConfigManager;
use singbox_api::controller_configuration::SingBoxControllerConfiguration;
use singbox_api::http::{
    SingBoxClashHttpPort, SingBoxClashMethod, SingBoxClashResponse, SingBoxControllerAuthorization,
};
use singbox_config::profile_document_codec::SingBoxProfileDocumentCodec;
use singbox_process::command::{NativeConfigCheck, SingBoxChildProcess, SingBoxCommandPort};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;

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

struct AcceptingSingBoxCommand;

#[async_trait]
impl SingBoxCommandPort for AcceptingSingBoxCommand {
    async fn check_config(
        &self,
        _binary_path: &Path,
        candidate_path: &Path,
    ) -> Result<NativeConfigCheck, PortError> {
        fs::read(candidate_path)
            .await
            .map_err(|error| PortError::Failed(format!("read staged sing-box JSON: {error}")))?;
        Ok(NativeConfigCheck::Accepted)
    }

    async fn spawn_core(
        &self,
        _binary_path: &Path,
        _config_path: &Path,
    ) -> Result<Box<dyn SingBoxChildProcess>, PortError> {
        Err(PortError::unsupported(
            Capability::CoreLifecycle,
            "profile-store conformance does not launch a Core process",
        ))
    }
}

struct UnusedSingBoxHttp;

#[async_trait]
impl SingBoxClashHttpPort for UnusedSingBoxHttp {
    async fn request(
        &self,
        _url: &url::Url,
        _method: SingBoxClashMethod,
        _authorization: &SingBoxControllerAuthorization,
        _body: Option<Vec<u8>>,
        _timeout: Duration,
    ) -> Result<SingBoxClashResponse, PortError> {
        Err(PortError::unsupported(
            Capability::RoutingIntent,
            "profile-store conformance does not dispatch controller requests",
        ))
    }
}

struct StoppedCoreSession {
    identity: CoreIdentity,
}

#[async_trait]
impl CoreLifecyclePort for StoppedCoreSession {
    fn core_identity(&self) -> Option<CoreIdentity> {
        Some(self.identity.clone())
    }

    fn lifecycle(&self) -> CoreLifecycle {
        CoreLifecycle::Stopped
    }

    fn generation(&self) -> u64 {
        0
    }

    fn session_token(&self) -> Option<SessionToken> {
        None
    }

    async fn start(&self) -> Result<u64, PortError> {
        Err(PortError::unsupported(
            Capability::CoreLifecycle,
            "profile-store conformance keeps the Core stopped",
        ))
    }

    async fn stop(&self) -> Result<(), PortError> {
        Ok(())
    }

    async fn restart(&self) -> Result<u64, PortError> {
        Err(PortError::unsupported(
            Capability::CoreLifecycle,
            "profile-store conformance keeps the Core stopped",
        ))
    }

    async fn wait_for_ready(&self, _generation: u64, _timeout: Duration) -> Result<(), PortError> {
        Err(PortError::unsupported(
            Capability::CoreLifecycle,
            "profile-store conformance does not start the Core",
        ))
    }
}

#[tokio::test]
async fn singbox_profile_store_and_native_transaction_share_engine_and_core_identity() {
    let home = TempDir::new().expect("temporary profile home");
    let store = Arc::new(
        ConfigManager::with_profile_codec_and_store(
            home.path().to_path_buf(),
            None,
            TestCredentialStore,
            Arc::new(SingBoxProfileDocumentCodec),
            ProfileDocumentExtension::Json,
        )
        .expect("production engine-scoped profile store"),
    );
    store
        .initialize_profile_namespace()
        .await
        .expect("initialize sing-box namespace");
    let profile_id = ProfileId::new("main").expect("valid profile id");
    let initial_document = br#"{"log":{"level":"warn"},"outbounds":[]}"#;
    ProfileStore::save_inactive_profile(
        store.engine_profile_store(),
        profile_id.as_str(),
        from_utf8(initial_document).expect("JSON is UTF-8"),
    )
    .await
    .expect("save initial sing-box profile");

    let initial_reference =
        ProfileStore::read_active_profile_reference(store.engine_profile_store())
            .await
            .expect("read product-wide reference");
    let selected_reference = match ProfileStore::compare_and_set_active_profile_reference(
        store.engine_profile_store(),
        initial_reference,
        ActiveProfileReferenceTarget::selected(store.engine_id(), profile_id.clone()),
        ActiveProfileReferenceWriteAuthorization::Unowned,
    )
    .await
    .expect("select sing-box profile through shared CAS")
    {
        ActiveProfileReferenceCasOutcome::Committed { current, .. } => match current {
            ActiveProfileReferenceState::Selected(reference) => reference,
            ActiveProfileReferenceState::Unselected { .. } => {
                panic!("committed profile reference must be selected")
            }
        },
        ActiveProfileReferenceCasOutcome::Stale { .. } => {
            panic!("isolated profile reference unexpectedly went stale")
        }
    };
    let before = ProfileStore::load_active_workspace(store.engine_profile_store())
        .await
        .expect("load selected workspace");
    assert_eq!(before.document.identity.engine_id, EngineId::sing_box());
    assert_eq!(before.document.raw_bytes, initial_document);

    let instance_id =
        CoreInstanceId::new("singbox-profile-store-conformance").expect("valid Core instance");
    let native_config_path = home.path().join("runtime/active.json");
    fs::create_dir_all(
        native_config_path
            .parent()
            .expect("native config has a parent directory"),
    )
    .await
    .expect("create native config directory");
    let provider = SingBoxEngineBackendProvider::new(
        home.path().join("bin/sing-box"),
        native_config_path.clone(),
        instance_id.clone(),
        SingBoxControllerConfiguration::from_profile_json(
            br#"{"experimental":{"clash_api":{"external_controller":"127.0.0.1:9090","secret":"test-token"}}}"#,
        )
        .expect("valid sing-box controller configuration"),
        Arc::new(AcceptingSingBoxCommand),
        Arc::new(UnusedSingBoxHttp),
    )
    .expect("compose production sing-box provider");
    let backend = provider.create_backend().expect("create sing-box ports");
    let session: Arc<dyn CoreLifecyclePort> = Arc::new(StoppedCoreSession {
        identity: CoreIdentity::new(EngineId::sing_box(), instance_id, None),
    });
    let profile_store: Arc<dyn ProfileStore> = store.shared_engine_profile_store();
    provider
        .bind_profile_configuration_transaction(&backend, profile_store, session)
        .expect("bind native transaction to the same engine and Core instance");

    let next_document =
        br#"{"log":{"level":"debug"},"outbounds":[{"type":"direct","tag":"direct"}]}"#;
    let next_sidecar = b"\x00opaque\xffsidecar\r\n".to_vec();
    let outcome = backend
        .configuration_transaction_binding()
        .transact(ProfileConfigurationTransactionRequest {
            expected_reference: selected_reference.clone(),
            expected: before.identity(),
            update: ProfileWorkspaceUpdate {
                purpose: ProfileWorkspacePurpose::Derived,
                document_bytes: next_document.to_vec(),
                sidecar_bytes: Some(next_sidecar.clone()),
            },
            authorization: ActiveProfileReferenceWriteAuthorization::Unowned,
            strategy: ApplyStrategy::PreferReload,
        })
        .await
        .expect("run native JSON + workspace transaction");
    let committed = match outcome {
        ProfileConfigurationTransactionOutcome::Committed { workspace, effect } => {
            assert_eq!(effect, ProfileConfigurationCommitEffect::StoredWhileStopped);
            workspace
        }
        ProfileConfigurationTransactionOutcome::RolledBack { cause, .. } => {
            panic!(
                "sing-box workspace transaction rolled back: {}",
                cause.message
            )
        }
        ProfileConfigurationTransactionOutcome::RollbackFailed { cause, rollback } => {
            panic!(
                "sing-box workspace transaction failed: {}; rollback: {}",
                cause.message, rollback.message
            )
        }
    };

    assert_eq!(committed.document.identity.engine_id, EngineId::sing_box());
    assert_eq!(committed.document.raw_bytes, next_document);
    assert_eq!(
        committed.sidecar_bytes.as_deref(),
        Some(next_sidecar.as_slice())
    );
    assert_eq!(
        fs::read(store.config_dir().join("main.json"))
            .await
            .expect("read engine-namespaced profile file"),
        next_document
    );
    assert_eq!(
        fs::read(native_config_path)
            .await
            .expect("read sing-box native config file"),
        next_document
    );
    assert_eq!(
        ProfileStore::read_active_profile_reference(store.engine_profile_store())
            .await
            .expect("read shared active reference"),
        ActiveProfileReferenceState::Selected(selected_reference)
    );
}
