use crate::store::EngineProfileStore;
use infiltrator_ports::secure_store::SecureStore;
use lurker_core_contract::active_profile_reference::{
    ActiveProfileReference, ActiveProfileReferenceRevision, ActiveProfileReferenceState,
};
use lurker_core_contract::identity::EngineId;
use lurker_core_contract::profile_document::DocumentValidation;
use lurker_core_contract::profile_document_identity::ProfileId;
use lurker_core_contract::profile_document_identity::{ProfileFormatId, ProfileSchemaVersion};
use lurker_core_ports::error::PortError;
use lurker_core_ports::profile_document_codec::{
    ProfileDocumentCodecPort, ProfileDocumentCodecScope,
};
use lurker_profile_store::store::ProfileDocumentExtension;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;

struct TestCodec(ProfileDocumentCodecScope);

impl ProfileDocumentCodecPort for TestCodec {
    fn scope(&self) -> ProfileDocumentCodecScope {
        self.0.clone()
    }

    fn inspect(&self, _document: &[u8]) -> Result<DocumentValidation, PortError> {
        Ok(DocumentValidation::Valid)
    }
}

struct NoopSecureStore;

#[async_trait::async_trait]
impl SecureStore for NoopSecureStore {
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

fn store(root: PathBuf, engine: &str) -> EngineProfileStore<NoopSecureStore> {
    let engine_id = EngineId::new(engine).unwrap();
    let codec = Arc::new(TestCodec(ProfileDocumentCodecScope {
        engine_id: engine_id.clone(),
        format_id: ProfileFormatId::new(format!("{engine}-native")).unwrap(),
        schema_version: ProfileSchemaVersion::new(1).unwrap(),
    }));
    EngineProfileStore::new(
        engine_id,
        root,
        codec,
        ProfileDocumentExtension::Json,
        NoopSecureStore,
    )
    .unwrap()
}

#[tokio::test]
async fn profile_catalog_projects_activity_only_for_the_selected_engine() {
    let root = TempDir::new().unwrap();
    let mihomo = store(root.path().to_path_buf(), "mihomo");
    let sing_box = store(root.path().to_path_buf(), "sing-box");
    let profile = ProfileId::new("main").unwrap();
    mihomo
        .write_profile_document_locked(&profile, b"mihomo")
        .await
        .unwrap();
    sing_box
        .write_profile_document_locked(&profile, b"sing-box")
        .await
        .unwrap();
    let selection = ActiveProfileReferenceState::Selected(ActiveProfileReference::new(
        EngineId::new("mihomo").unwrap(),
        profile,
        ActiveProfileReferenceRevision::new(7),
    ));

    let mihomo_guard = mihomo.lock_profile_writes().await.unwrap();
    let mihomo_profiles = mihomo.list_profile_info_locked(&selection).await.unwrap();
    drop(mihomo_guard);
    let sing_box_guard = sing_box.lock_profile_writes().await.unwrap();
    let sing_box_profiles = sing_box.list_profile_info_locked(&selection).await.unwrap();
    drop(sing_box_guard);

    assert_eq!(mihomo_profiles.len(), 1);
    assert!(mihomo_profiles[0].active);
    let metadata = mihomo.get_profile_metadata("main").await.unwrap();
    assert!(metadata.auto_reload_core);
    assert_eq!(
        mihomo_profiles[0].auto_reload_core,
        metadata.auto_reload_core
    );
    assert_eq!(sing_box_profiles.len(), 1);
    assert!(!sing_box_profiles[0].active);
    assert_ne!(mihomo_profiles[0].path, sing_box_profiles[0].path);
}
