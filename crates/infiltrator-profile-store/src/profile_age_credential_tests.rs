use super::age_credential_key;
use crate::store::{EngineProfileStore, ProfileDocumentExtension};
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReferenceRevision, ActiveProfileReferenceState,
    ActiveProfileReferenceWriteAuthorization,
};
use infiltrator_contract::core_identity::{CoreGeneration, CoreIdentity, CoreInstanceId, EngineId};
use infiltrator_contract::profile_age_credential::{
    AgePrivateKey, ProfileAgeCredentialMutation, ProfileAgeCredentialObservation,
    ProfileAgeCredentialReadOutcome, ProfileAgeCredentialReadRequest, ProfileAgeCredentialRevision,
    ProfileAgeCredentialState, ProfileAgeCredentialTarget, ProfileAgeCredentialWriteRequest,
};
use infiltrator_contract::profile_document::DocumentValidation;
use infiltrator_contract::profile_document_identity::{
    ProfileFormatId, ProfileId, ProfileSchemaVersion,
};
use infiltrator_domain::profiles::ProfileMetadata;
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
use tokio::fs::read_to_string;

struct TestCodec(ProfileDocumentCodecScope);

impl ProfileDocumentCodecPort for TestCodec {
    fn scope(&self) -> ProfileDocumentCodecScope {
        self.0.clone()
    }

    fn inspect(&self, _document: &[u8]) -> Result<DocumentValidation, PortError> {
        Ok(DocumentValidation::Valid)
    }
}

#[derive(Clone)]
enum FakeSecureStoreMode {
    Healthy,
    DenyWrite(String),
    IgnoreWrite,
}

struct MemorySecureStore {
    values: Arc<Mutex<HashMap<(String, String), String>>>,
    mode: FakeSecureStoreMode,
}

impl MemorySecureStore {
    fn healthy() -> Self {
        Self {
            values: Arc::new(Mutex::new(HashMap::new())),
            mode: FakeSecureStoreMode::Healthy,
        }
    }

    fn with_mode(mode: FakeSecureStoreMode) -> Self {
        Self {
            values: Arc::new(Mutex::new(HashMap::new())),
            mode,
        }
    }

    fn contains_value(&self, value: &str) -> bool {
        self.values
            .lock()
            .expect("secure store lock")
            .values()
            .any(|stored| stored.contains(value))
    }
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
        match &self.mode {
            FakeSecureStoreMode::Healthy => {
                self.values
                    .lock()
                    .expect("secure store lock")
                    .insert((namespace.to_owned(), key.to_owned()), value.to_owned());
                Ok(())
            }
            FakeSecureStoreMode::DenyWrite(message) => {
                Err(PortError::PermissionDenied(message.clone()))
            }
            FakeSecureStoreMode::IgnoreWrite => Ok(()),
        }
    }

    async fn delete(&self, namespace: &str, key: &str) -> Result<(), PortError> {
        self.values
            .lock()
            .expect("secure store lock")
            .remove(&(namespace.to_owned(), key.to_owned()));
        Ok(())
    }
}

fn make_store(
    root: PathBuf,
    secure_store: MemorySecureStore,
) -> EngineProfileStore<MemorySecureStore> {
    let engine_id = EngineId::mihomo();
    let codec = Arc::new(TestCodec(ProfileDocumentCodecScope {
        engine_id: engine_id.clone(),
        format_id: ProfileFormatId::new("mihomo-native").unwrap(),
        schema_version: ProfileSchemaVersion::new(1).unwrap(),
    }));
    EngineProfileStore::new(
        engine_id,
        root,
        codec,
        ProfileDocumentExtension::Json,
        secure_store,
    )
    .unwrap()
}

fn read_request(profile_id: &ProfileId) -> ProfileAgeCredentialReadRequest {
    ProfileAgeCredentialReadRequest {
        profile_id: profile_id.clone(),
        expected_selection: ActiveProfileReferenceState::default(),
        authorization: ActiveProfileReferenceWriteAuthorization::Unowned,
    }
}

fn observation(
    profile_id: &ProfileId,
    revision: u64,
    state: ProfileAgeCredentialState,
) -> ProfileAgeCredentialObservation {
    ProfileAgeCredentialObservation::new(
        EngineId::mihomo(),
        profile_id.clone(),
        ProfileAgeCredentialRevision::new(revision),
        state,
    )
}

fn write_request(
    expected: ProfileAgeCredentialObservation,
    target: ProfileAgeCredentialTarget,
) -> ProfileAgeCredentialWriteRequest {
    ProfileAgeCredentialWriteRequest {
        expected,
        expected_selection: ActiveProfileReferenceState::default(),
        authorization: ActiveProfileReferenceWriteAuthorization::Unowned,
        target,
    }
}

async fn create_profile(store: &EngineProfileStore<MemorySecureStore>, profile_id: &ProfileId) {
    store
        .write_profile_document_locked(profile_id, b"profile document")
        .await
        .expect("profile document");
    store
        .update_profile_metadata(profile_id.as_str(), &ProfileMetadata::default())
        .await
        .expect("profile metadata");
}

#[tokio::test]
async fn age_credential_is_secure_revisioned_and_absent_from_profile_read_models() {
    let root = TempDir::new().expect("config root");
    let secure_store = MemorySecureStore::healthy();
    let secure_store_probe = MemorySecureStore {
        values: Arc::clone(&secure_store.values),
        mode: FakeSecureStoreMode::Healthy,
    };
    let store = make_store(root.path().to_path_buf(), secure_store);
    let profile_id = ProfileId::new("primary").unwrap();
    create_profile(&store, &profile_id).await;

    let readback = ProfileStore::profile_age_credential(&store, read_request(&profile_id))
        .await
        .expect("initial read");
    let ProfileAgeCredentialReadOutcome::Observed { current } = readback else {
        panic!("expected current owner readback")
    };
    assert_eq!(
        current,
        observation(&profile_id, 0, ProfileAgeCredentialState::NotConfigured)
    );

    let secret = "AGE-SECRET-KEY-owner-only";
    let update = ProfileStore::mutate_profile_age_credential(
        &store,
        write_request(
            current,
            ProfileAgeCredentialTarget::Set(AgePrivateKey::new(secret.to_owned()).unwrap()),
        ),
    )
    .await
    .expect("secure update");
    assert!(matches!(
        update,
        ProfileAgeCredentialMutation::Updated { ref current, .. }
            if current == &observation(&profile_id, 1, ProfileAgeCredentialState::Configured)
    ));
    assert!(secure_store_probe.contains_value(secret));
    assert!(!format!("{update:?}").contains(secret));

    let profile_readback = ProfileStore::list_profiles(&store).await.unwrap();
    assert!(!format!("{profile_readback:?}").contains(secret));
    let metadata = ProfileStore::get_profile_metadata(&store, profile_id.as_str())
        .await
        .unwrap();
    assert!(!format!("{metadata:?}").contains(secret));
    let disk_state = read_to_string(store.settings_file())
        .await
        .unwrap_or_default();
    assert!(!disk_state.contains(secret));
}

#[tokio::test]
async fn age_credential_clear_is_revisioned_and_stale_edits_have_no_effect() {
    let root = TempDir::new().expect("config root");
    let store = make_store(root.path().to_path_buf(), MemorySecureStore::healthy());
    let profile_id = ProfileId::new("primary").unwrap();
    create_profile(&store, &profile_id).await;

    let initial = observation(&profile_id, 0, ProfileAgeCredentialState::NotConfigured);
    let configured = ProfileStore::mutate_profile_age_credential(
        &store,
        write_request(
            initial.clone(),
            ProfileAgeCredentialTarget::Set(
                AgePrivateKey::new("AGE-SECRET-KEY-one".into()).unwrap(),
            ),
        ),
    )
    .await
    .unwrap();
    let ProfileAgeCredentialMutation::Updated { current, .. } = configured else {
        panic!("expected configured readback")
    };

    let stale = ProfileStore::mutate_profile_age_credential(
        &store,
        write_request(initial, ProfileAgeCredentialTarget::Clear),
    )
    .await
    .unwrap();
    assert!(matches!(
        stale,
        ProfileAgeCredentialMutation::StaleCredential { .. }
    ));

    let cleared = ProfileStore::mutate_profile_age_credential(
        &store,
        write_request(current, ProfileAgeCredentialTarget::Clear),
    )
    .await
    .unwrap();
    assert!(matches!(
        cleared,
        ProfileAgeCredentialMutation::Cleared { ref current, .. }
            if current == &observation(&profile_id, 2, ProfileAgeCredentialState::NotConfigured)
    ));
    assert_eq!(
        ProfileStore::profile_age_credential(&store, read_request(&profile_id))
            .await
            .unwrap(),
        ProfileAgeCredentialReadOutcome::Observed {
            current: observation(&profile_id, 2, ProfileAgeCredentialState::NotConfigured)
        }
    );
}

#[tokio::test]
async fn age_credential_owner_rejects_stale_selection_and_wrong_core_authorization() {
    let root = TempDir::new().expect("config root");
    let store = make_store(root.path().to_path_buf(), MemorySecureStore::healthy());
    let profile_id = ProfileId::new("primary").unwrap();
    create_profile(&store, &profile_id).await;

    let mut stale_request = read_request(&profile_id);
    stale_request.expected_selection = ActiveProfileReferenceState::Unselected {
        revision: ActiveProfileReferenceRevision::new(3),
    };
    assert!(matches!(
        ProfileStore::profile_age_credential(&store, stale_request)
            .await
            .unwrap(),
        ProfileAgeCredentialReadOutcome::StaleSelection { .. }
    ));

    let mut unauthorized = read_request(&profile_id);
    unauthorized.authorization = ActiveProfileReferenceWriteAuthorization::BoundOwner {
        identity: CoreIdentity::new(
            EngineId::mihomo(),
            CoreInstanceId::new("test-owner").unwrap(),
            None,
        ),
        generation: CoreGeneration::new(1),
    };
    let error = ProfileStore::profile_age_credential(&store, unauthorized)
        .await
        .expect_err("unowned store rejects forged owner authorization");
    assert!(!error.to_string().contains("AGE-SECRET-KEY"));
}

#[tokio::test]
async fn age_credential_secure_store_failures_are_sanitized_and_readback_is_required() {
    let secret = "AGE-SECRET-KEY-never-in-error";
    let root = TempDir::new().expect("config root");
    let store = make_store(
        root.path().to_path_buf(),
        MemorySecureStore::with_mode(FakeSecureStoreMode::DenyWrite(secret.to_owned())),
    );
    let profile_id = ProfileId::new("primary").unwrap();
    create_profile(&store, &profile_id).await;
    let error = ProfileStore::mutate_profile_age_credential(
        &store,
        write_request(
            observation(&profile_id, 0, ProfileAgeCredentialState::NotConfigured),
            ProfileAgeCredentialTarget::Set(AgePrivateKey::new(secret.to_owned()).unwrap()),
        ),
    )
    .await
    .expect_err("secure storage denial");
    assert!(!error.to_string().contains(secret));

    let root = TempDir::new().expect("config root");
    let store = make_store(
        root.path().to_path_buf(),
        MemorySecureStore::with_mode(FakeSecureStoreMode::IgnoreWrite),
    );
    let profile_id = ProfileId::new("primary").unwrap();
    create_profile(&store, &profile_id).await;
    let error = ProfileStore::mutate_profile_age_credential(
        &store,
        write_request(
            observation(&profile_id, 0, ProfileAgeCredentialState::NotConfigured),
            ProfileAgeCredentialTarget::Set(AgePrivateKey::new(secret.to_owned()).unwrap()),
        ),
    )
    .await
    .expect_err("missing secure-store readback");
    assert!(error.to_string().contains("mismatched readback"));
    assert!(!error.to_string().contains(secret));
}

#[tokio::test]
async fn age_credential_secure_entry_is_engine_scoped() {
    assert_ne!(
        age_credential_key(&EngineId::mihomo(), "primary"),
        age_credential_key(&EngineId::sing_box(), "primary")
    );
}
