use crate::store::{EngineProfileStore, ProfileDocumentExtension};
use infiltrator_contract::core_identity::EngineId;
use infiltrator_contract::profile_document_identity::ProfileId;
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_document_codec::{
    ProfileDocumentCodecPort, ProfileDocumentCodecScope,
};
use infiltrator_ports::secure_store::SecureStore;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;

struct TestCodec(ProfileDocumentCodecScope);

impl ProfileDocumentCodecPort for TestCodec {
    fn scope(&self) -> ProfileDocumentCodecScope {
        self.0.clone()
    }

    fn inspect(
        &self,
        _document: &[u8],
    ) -> Result<infiltrator_contract::profile_document::DocumentValidation, PortError> {
        Ok(infiltrator_contract::profile_document::DocumentValidation::Valid)
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

fn store(
    root: PathBuf,
    engine: &str,
    extension: ProfileDocumentExtension,
) -> EngineProfileStore<NoopSecureStore> {
    let engine_id = EngineId::new(engine).unwrap();
    let codec = Arc::new(TestCodec(ProfileDocumentCodecScope {
        engine_id: engine_id.clone(),
        format_id: infiltrator_contract::profile_document_identity::ProfileFormatId::new(format!(
            "{engine}-native"
        ))
        .unwrap(),
        schema_version: infiltrator_contract::profile_document_identity::ProfileSchemaVersion::new(
            1,
        )
        .unwrap(),
    }));
    EngineProfileStore::new(engine_id, root, codec, extension, NoopSecureStore).unwrap()
}

#[tokio::test]
async fn document_paths_are_engine_scoped_and_use_the_injected_extension() {
    let root = TempDir::new().unwrap();
    let yaml_store = store(
        root.path().to_path_buf(),
        "mihomo",
        ProfileDocumentExtension::Yaml,
    );
    let json_store = store(
        root.path().to_path_buf(),
        "sing-box",
        ProfileDocumentExtension::Json,
    );
    let profile = ProfileId::new("main").unwrap();

    let yaml = yaml_store.profile_document_path(&profile).await.unwrap();
    let json = json_store.profile_document_path(&profile).await.unwrap();

    assert_eq!(yaml, root.path().join("engines/mihomo/main.yaml"));
    assert_eq!(json, root.path().join("engines/sing-box/main.json"));
}

#[tokio::test]
async fn document_paths_reject_relative_segments_before_access() {
    let root = TempDir::new().unwrap();
    let store = store(
        root.path().to_path_buf(),
        "mihomo",
        ProfileDocumentExtension::Yaml,
    );

    for name in ["release..backup", "name.", "..leading"] {
        let error = store
            .profile_document_path(&ProfileId::new(name).unwrap())
            .await
            .unwrap_err();
        assert!(matches!(error, PortError::Rejected(_)));
    }
    assert!(!root.path().join("engines").exists());
}

#[tokio::test]
async fn existing_document_path_reports_missing_document_and_rejects_escape_symlink() {
    let root = TempDir::new().unwrap();
    let store = store(
        root.path().to_path_buf(),
        "mihomo",
        ProfileDocumentExtension::Yaml,
    );
    let profile = ProfileId::new("main").unwrap();

    assert!(matches!(
        store.existing_profile_document_path(&profile).await,
        Err(PortError::NotFound(_))
    ));

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let document = store.profile_document_path(&profile).await.unwrap();
        let outside = root.path().join("outside.yaml");
        std::fs::write(&outside, "external").unwrap();
        symlink(&outside, &document).unwrap();
        assert!(matches!(
            store.existing_profile_document_path(&profile).await,
            Err(PortError::Rejected(_))
        ));
    }
}
