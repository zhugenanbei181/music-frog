use super::super::core_application_for_singbox;
use super::{
    TestBridge, TestCredentialStore, TestLifecycleCapabilityProvider, TestReadiness, config_manager,
};
use async_trait::async_trait;
use infiltrator_contract::core_capabilities::EngineBinaryTarget;
use infiltrator_contract::core_identity::EngineId;
use infiltrator_contract::profile_document_identity::ProfileId;
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_ports::core_binary_selection::{
    CoreBinarySelection, CoreBinarySelectionPort, CoreBinarySelectionRequest,
};
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_store::ProfileStore;
use mihomo_config::manager::{ConfigManager, ProfileDocumentExtension};
use singbox_adapter::profile_document_codec::SingBoxProfileDocumentCodec;
use std::sync::Arc;

const PROFILE: &str = r#"{"experimental":{"clash_api":{"external_controller":"127.0.0.1:9090"}},"inbounds":[],"outbounds":[]}"#;

struct UnavailableBinarySelection;

#[async_trait]
impl CoreBinarySelectionPort for UnavailableBinarySelection {
    fn engine_id(&self) -> EngineId {
        singbox_adapter::engine_id()
    }

    async fn select(
        &self,
        _request: CoreBinarySelectionRequest,
    ) -> Result<CoreBinarySelection, PortError> {
        Err(PortError::unsupported(
            infiltrator_contract::capability::Capability::CoreLifecycle,
            "composition identity test does not select or execute a binary",
        ))
    }
}

#[tokio::test]
async fn singbox_core_composition_keeps_the_engine_codec_and_owner_identity_aligned() {
    let (home, _mihomo_store) = config_manager();
    let profile_store = Arc::new(
        ConfigManager::with_profile_codec_and_store(
            home.0.clone(),
            None,
            TestCredentialStore,
            Arc::new(SingBoxProfileDocumentCodec),
            ProfileDocumentExtension::Json,
        )
        .expect("sing-box profile manager"),
    );
    profile_store
        .initialize_profile_namespace()
        .await
        .expect("initialize sing-box profile namespace");
    ProfileStore::save_inactive_profile(profile_store.as_ref(), "main", PROFILE)
        .await
        .expect("save sing-box profile");
    let profile_id = ProfileId::new("main").expect("profile ID");
    let document = ProfileStore::load_workspace(profile_store.as_ref(), &profile_id)
        .await
        .expect("load exact sing-box profile")
        .document;

    let application = core_application_for_singbox(
        TestBridge::new(),
        profile_store,
        document,
        Arc::new(UnavailableBinarySelection),
        home.0.clone(),
        EngineBinaryTarget::Aarch64LinuxAndroid,
        Arc::new(TestReadiness),
        Arc::new(TestLifecycleCapabilityProvider),
    )
    .expect("compose lifecycle owner without selecting a test binary");

    assert_eq!(
        application
            .snapshot()
            .identity
            .expect("Core identity")
            .engine,
        singbox_adapter::engine_id()
    );
    assert_eq!(application.snapshot().lifecycle, CoreLifecycle::Stopped);
}
