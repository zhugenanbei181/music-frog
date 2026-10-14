use crate::profile_files::profile_document_backup_path;
use crate::store::{EngineProfileStore as Store, ProfileDocumentExtension};
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReferenceState, ActiveProfileReferenceWriteAuthorization,
};
use infiltrator_contract::core_identity::EngineId;
use infiltrator_contract::profile_age_credential::{
    AgePrivateKey, ProfileAgeCredentialReadOutcome, ProfileAgeCredentialReadRequest,
    ProfileAgeCredentialState, ProfileAgeCredentialTarget, ProfileAgeCredentialWriteRequest,
};
use infiltrator_contract::profile_document::DocumentValidation;
use infiltrator_contract::profile_document_identity::{
    ProfileFormatId, ProfileId, ProfileSchemaVersion,
};
use infiltrator_domain::profile_options::options_path;
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_document_codec::{
    ProfileDocumentCodecPort, ProfileDocumentCodecScope,
};
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::secure_store::SecureStore;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
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

#[derive(Clone, Default)]
struct MemorySecureStore {
    values: Arc<Mutex<HashMap<(String, String), String>>>,
}

#[async_trait::async_trait]
impl SecureStore for MemorySecureStore {
    async fn get(&self, namespace: &str, key: &str) -> Result<Option<String>, PortError> {
        Ok(self
            .values
            .lock()
            .expect("secure store lock")
            .get(&(namespace.to_owned(), key.to_owned()))
            .cloned())
    }

    async fn set(&self, namespace: &str, key: &str, value: &str) -> Result<(), PortError> {
        self.values
            .lock()
            .expect("secure store lock")
            .insert((namespace.to_owned(), key.to_owned()), value.to_owned());
        Ok(())
    }

    async fn delete(&self, namespace: &str, key: &str) -> Result<(), PortError> {
        self.values
            .lock()
            .expect("secure store lock")
            .remove(&(namespace.to_owned(), key.to_owned()));
        Ok(())
    }
}

impl MemorySecureStore {
    fn contains_value(&self, value: &str) -> bool {
        self.values
            .lock()
            .expect("secure store lock")
            .values()
            .any(|stored| stored.contains(value))
    }
}

fn store(root: PathBuf, secure_store: MemorySecureStore) -> Store<MemorySecureStore> {
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
        secure_store,
    )
    .unwrap()
}

#[tokio::test]
async fn delete_profile_locked_removes_document_sidecar_and_backup() {
    let directory = TempDir::new().unwrap();
    let store = store(directory.path().to_path_buf(), MemorySecureStore::default());
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
    let secure_store = MemorySecureStore::default();
    let secure_store_probe = secure_store.clone();
    let store = store(directory.path().to_path_buf(), secure_store);
    let profile = ProfileId::new("primary").unwrap();
    let document = store.config_dir().join("primary.yaml");
    let sidecar = options_path(store.config_dir(), profile.as_str());
    let previous = b"mode: rule\n";

    store
        .write_profile_document_locked(&profile, previous)
        .await
        .unwrap();
    let ProfileAgeCredentialReadOutcome::Observed { current } =
        ProfileStore::profile_age_credential(
            &store,
            ProfileAgeCredentialReadRequest {
                profile_id: profile.clone(),
                expected_selection: ActiveProfileReferenceState::default(),
                authorization: ActiveProfileReferenceWriteAuthorization::Unowned,
            },
        )
        .await
        .unwrap()
    else {
        panic!("expected current credential readback")
    };
    let secret = "AGE-SECRET-KEY-delete-rollback";
    ProfileStore::mutate_profile_age_credential(
        &store,
        ProfileAgeCredentialWriteRequest {
            expected: current,
            expected_selection: ActiveProfileReferenceState::default(),
            authorization: ActiveProfileReferenceWriteAuthorization::Unowned,
            target: ProfileAgeCredentialTarget::Set(AgePrivateKey::new(secret.to_owned()).unwrap()),
        },
    )
    .await
    .unwrap();
    assert!(secure_store_probe.contains_value(secret));

    fs::create_dir_all(&sidecar).await.unwrap();
    fs::write(sidecar.join("blocks-removal"), b"keep directory non-empty")
        .await
        .unwrap();

    assert!(store.delete_profile_locked(&profile).await.is_err());
    assert_eq!(fs::read(document).await.unwrap(), previous);
    assert!(secure_store_probe.contains_value(secret));
    let ProfileAgeCredentialReadOutcome::Observed { current } =
        ProfileStore::profile_age_credential(
            &store,
            ProfileAgeCredentialReadRequest {
                profile_id: profile.clone(),
                expected_selection: ActiveProfileReferenceState::default(),
                authorization: ActiveProfileReferenceWriteAuthorization::Unowned,
            },
        )
        .await
        .unwrap()
    else {
        panic!("expected restored credential readback")
    };
    assert_eq!(current.revision.value(), 1);
    assert_eq!(current.state, ProfileAgeCredentialState::Configured);
}

#[tokio::test]
async fn profile_document_backup_restores_exact_prior_bytes() {
    let root = TempDir::new().unwrap();
    let store = store(root.path().to_path_buf(), MemorySecureStore::default());
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
