use crate::profile_files::profile_document_backup_path;
use crate::store::{EngineProfileStore as Store, ProfileDocumentExtension};
use infiltrator_contract::core_identity::EngineId;
use infiltrator_contract::profile_document::DocumentValidation;
use infiltrator_contract::profile_document_identity::{
    ProfileFormatId, ProfileId, ProfileSchemaVersion,
};
use infiltrator_domain::profile_options::options_path;
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_document_codec::{
    ProfileDocumentCodecPort, ProfileDocumentCodecScope,
};
use infiltrator_ports::secure_store::SecureStore;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::fs;

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

fn store(root: PathBuf) -> Store<NoopSecureStore> {
    let engine = EngineId::new("mihomo").unwrap();
    let codec = Arc::new(TestCodec(ProfileDocumentCodecScope {
        engine_id: engine.clone(),
        format_id: ProfileFormatId::new("mihomo-yaml").unwrap(),
        schema_version: ProfileSchemaVersion::new(1).unwrap(),
    }));
    Store::new(
        engine,
        root,
        codec,
        ProfileDocumentExtension::Yaml,
        NoopSecureStore,
    )
    .unwrap()
}

#[tokio::test]
async fn delete_profile_locked_removes_document_sidecar_and_backup() {
    let directory = TempDir::new().unwrap();
    let store = store(directory.path().to_path_buf());
    let profile = ProfileId::new("primary").unwrap();
    let document = store.config_dir().join("primary.yaml");
    let backup = profile_document_backup_path(&document);
    let sidecar = options_path(store.config_dir(), profile.as_str());

    store
        .write_profile_document_locked(&profile, b"mode: rule\n")
        .await
        .unwrap();
    fs::write(&backup, b"old document\n").await.unwrap();
    fs::create_dir_all(sidecar.parent().unwrap()).await.unwrap();
    fs::write(&sidecar, b"profile options\n").await.unwrap();

    store.delete_profile_locked(&profile).await.unwrap();

    assert!(!document.exists());
    assert!(!backup.exists());
    assert!(!sidecar.exists());
}

#[tokio::test]
async fn delete_profile_locked_restores_document_when_sidecar_removal_fails() {
    let directory = TempDir::new().unwrap();
    let store = store(directory.path().to_path_buf());
    let profile = ProfileId::new("primary").unwrap();
    let document = store.config_dir().join("primary.yaml");
    let sidecar = options_path(store.config_dir(), profile.as_str());
    let previous = b"mode: rule\n";

    store
        .write_profile_document_locked(&profile, previous)
        .await
        .unwrap();
    fs::create_dir_all(&sidecar).await.unwrap();
    fs::write(sidecar.join("blocks-removal"), b"keep directory non-empty")
        .await
        .unwrap();

    assert!(store.delete_profile_locked(&profile).await.is_err());
    assert_eq!(fs::read(document).await.unwrap(), previous);
}

#[tokio::test]
async fn profile_document_backup_restores_exact_prior_bytes() {
    let root = TempDir::new().unwrap();
    let store = store(root.path().to_path_buf());
    let profile = ProfileId::new("main").unwrap();

    assert!(!store.profile_document_has_backup(&profile).await.unwrap());
    store
        .write_profile_document_locked(&profile, b"port: 7890\n")
        .await
        .unwrap();
    store
        .write_profile_document_locked(&profile, b"port: 7891\n")
        .await
        .unwrap();

    assert!(store.profile_document_has_backup(&profile).await.unwrap());
    assert_eq!(
        store
            .read_profile_document_backup(&profile)
            .await
            .unwrap()
            .as_deref(),
        Some("port: 7890\n")
    );
    assert!(
        store
            .restore_profile_document_backup_locked(&profile)
            .await
            .unwrap()
    );
    assert_eq!(
        store.read_profile_document(&profile).await.unwrap(),
        "port: 7890\n"
    );
    store.clear_profile_document_backup(&profile).await.unwrap();
    assert!(!store.profile_document_has_backup(&profile).await.unwrap());
}
