use super::read_credential_record;
use crate::store::EngineProfileStore;
use age::secrecy::ExposeSecret;
use age::x25519::Identity;
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReference, ActiveProfileReferenceRevision, ActiveProfileReferenceState,
    ActiveProfileReferenceTarget, ActiveProfileReferenceWriteAuthorization,
};
use infiltrator_contract::profile_age_credential::{
    AgePrivateKey, ProfileAgeContentDecryption, ProfileAgeCredentialMutation,
    ProfileAgeCredentialObservation, ProfileAgeCredentialReadOutcome,
    ProfileAgeCredentialReadRequest, ProfileAgeCredentialRevision, ProfileAgeCredentialState,
    ProfileAgeCredentialTarget, ProfileAgeCredentialWriteRequest, ProfileAgeEncryptedContent,
};
use infiltrator_contract::profile_document_identity::ProfileId;
use infiltrator_domain::profiles::ProfileMetadata;
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::secure_store::SecureStore;
use lurker_core_contract::identity::{CoreGeneration, CoreIdentity, CoreInstanceId, EngineId};
use lurker_core_contract::profile_document::DocumentValidation;
use lurker_core_contract::profile_document_identity::{ProfileFormatId, ProfileSchemaVersion};
use lurker_core_ports::error::PortError;
use lurker_core_ports::profile_document_codec::{
    ProfileDocumentCodecPort, ProfileDocumentCodecScope,
};
use lurker_profile_store::store::ProfileDocumentExtension;
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
    make_store_for(root, EngineId::mihomo(), secure_store)
}

fn make_store_for(
    root: PathBuf,
    engine_id: EngineId,
    secure_store: MemorySecureStore,
) -> EngineProfileStore<MemorySecureStore> {
    let codec = Arc::new(TestCodec(ProfileDocumentCodecScope {
        engine_id: engine_id.clone(),
        format_id: ProfileFormatId::new(format!("{}-native", engine_id.as_str())).unwrap(),
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
    observation_for(EngineId::mihomo(), profile_id, revision, state)
}

fn observation_for(
    engine_id: EngineId,
    profile_id: &ProfileId,
    revision: u64,
    state: ProfileAgeCredentialState,
) -> ProfileAgeCredentialObservation {
    ProfileAgeCredentialObservation::new(
        engine_id,
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
    let ProfileAgeCredentialReadOutcome::Observed { current, selection } = readback else {
        panic!("expected current owner readback")
    };
    assert_eq!(selection, ActiveProfileReferenceState::default());
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
            current: observation(&profile_id, 2, ProfileAgeCredentialState::NotConfigured),
            selection: ActiveProfileReferenceState::default(),
        }
    );
}

#[tokio::test]
async fn age_credential_readback_returns_the_exact_selected_reference_revision() {
    let root = TempDir::new().expect("config root");
    let store = make_store(root.path().to_path_buf(), MemorySecureStore::healthy());
    let profile_id = ProfileId::new("primary").unwrap();
    create_profile(&store, &profile_id).await;

    ProfileStore::compare_and_set_active_profile_reference(
        &store,
        ActiveProfileReferenceState::default(),
        ActiveProfileReferenceTarget::selected(EngineId::mihomo(), profile_id.clone()),
        ActiveProfileReferenceWriteAuthorization::Unowned,
    )
    .await
    .expect("select profile");
    let selection = ProfileStore::read_active_profile_reference(&store)
        .await
        .expect("read selected profile");

    let readback = ProfileStore::profile_age_credential(
        &store,
        ProfileAgeCredentialReadRequest {
            profile_id: profile_id.clone(),
            expected_selection: selection.clone(),
            authorization: ActiveProfileReferenceWriteAuthorization::Unowned,
        },
    )
    .await
    .expect("read owner credential state");

    assert_eq!(
        readback,
        ProfileAgeCredentialReadOutcome::Observed {
            current: observation(&profile_id, 0, ProfileAgeCredentialState::NotConfigured),
            selection: ActiveProfileReferenceState::Selected(ActiveProfileReference::new(
                EngineId::mihomo(),
                profile_id,
                selection.revision(),
            )),
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
    let root = TempDir::new().expect("config root");
    let shared_store = MemorySecureStore::healthy();
    let shared_probe = MemorySecureStore {
        values: Arc::clone(&shared_store.values),
        mode: FakeSecureStoreMode::Healthy,
    };
    let mihomo = make_store_for(root.path().to_path_buf(), EngineId::mihomo(), shared_store);
    let sing_box = make_store_for(
        root.path().to_path_buf(),
        EngineId::sing_box(),
        MemorySecureStore {
            values: Arc::clone(&shared_probe.values),
            mode: FakeSecureStoreMode::Healthy,
        },
    );
    let profile_id = ProfileId::new("primary").unwrap();
    create_profile(&mihomo, &profile_id).await;
    create_profile(&sing_box, &profile_id).await;

    ProfileStore::mutate_profile_age_credential(
        &mihomo,
        write_request(
            observation(&profile_id, 0, ProfileAgeCredentialState::NotConfigured),
            ProfileAgeCredentialTarget::Set(
                AgePrivateKey::new("AGE-SECRET-KEY-mihomo-only".to_owned()).unwrap(),
            ),
        ),
    )
    .await
    .expect("Mihomo credential");

    let observation = ProfileStore::profile_age_credential(&sing_box, read_request(&profile_id))
        .await
        .expect("sing-box credential state");
    assert_eq!(
        observation,
        ProfileAgeCredentialReadOutcome::Observed {
            current: ProfileAgeCredentialObservation::new(
                EngineId::sing_box(),
                profile_id.clone(),
                ProfileAgeCredentialRevision::new(0),
                ProfileAgeCredentialState::NotConfigured,
            ),
            selection: ActiveProfileReferenceState::default(),
        }
    );
    assert!(shared_probe.contains_value("AGE-SECRET-KEY-mihomo-only"));
}

#[tokio::test]
async fn age_subscription_payload_decrypts_only_inside_profile_owner() {
    let root = TempDir::new().expect("config root");
    let store = make_store(root.path().to_path_buf(), MemorySecureStore::healthy());
    let profile_id = ProfileId::new("primary").unwrap();
    create_profile(&store, &profile_id).await;

    let identity = Identity::generate();
    let private_key = identity.to_string();
    let private_key_text = private_key.expose_secret().to_owned();
    let encrypted = age::encrypt_and_armor(&identity.to_public(), b"proxies: []\n")
        .expect("armored age document");

    assert!(matches!(
        store
            .decrypt_age_subscription_content(
                &profile_id,
                ProfileAgeEncryptedContent::new(encrypted.clone()).unwrap(),
            )
            .await
            .unwrap(),
        ProfileAgeContentDecryption::CredentialNotConfigured
    ));

    let configured = ProfileStore::mutate_profile_age_credential(
        &store,
        write_request(
            observation(&profile_id, 0, ProfileAgeCredentialState::NotConfigured),
            ProfileAgeCredentialTarget::Set(AgePrivateKey::new(private_key_text).unwrap()),
        ),
    )
    .await
    .expect("configure key");
    let ProfileAgeCredentialMutation::Updated { current, .. } = configured else {
        panic!("expected configured credential")
    };

    let decrypted = store
        .decrypt_age_subscription_content(
            &profile_id,
            ProfileAgeEncryptedContent::new(encrypted.clone()).unwrap(),
        )
        .await
        .expect("decrypt");
    let ProfileAgeContentDecryption::Decrypted {
        content,
        credential_revision,
    } = decrypted
    else {
        panic!("expected decrypted profile content")
    };
    assert_eq!(content.as_str(), "proxies: []\n");
    assert_eq!(credential_revision, ProfileAgeCredentialRevision::new(1));
    assert!(!format!("{content:?}").contains("proxies: []"));

    let wrong_identity = Identity::generate();
    let wrong_key = wrong_identity.to_string();
    let current = ProfileStore::mutate_profile_age_credential(
        &store,
        write_request(
            current,
            ProfileAgeCredentialTarget::Set(
                AgePrivateKey::new(wrong_key.expose_secret().to_owned()).unwrap(),
            ),
        ),
    )
    .await
    .expect("replace key");
    assert!(matches!(
        current,
        ProfileAgeCredentialMutation::Updated { .. }
    ));
    assert!(matches!(
        store
            .decrypt_age_subscription_content(
                &profile_id,
                ProfileAgeEncryptedContent::new(encrypted).unwrap(),
            )
            .await
            .unwrap(),
        ProfileAgeContentDecryption::InvalidCredentialOrContent
    ));
}

#[tokio::test]
async fn age_subscription_can_preconfigure_a_new_profile_before_import() {
    let root = TempDir::new().expect("config root");
    let store = make_store(root.path().to_path_buf(), MemorySecureStore::healthy());
    let profile_id = ProfileId::new("fresh-encrypted").unwrap();
    assert!(
        store
            .existing_profile_document_path(&profile_id)
            .await
            .is_err()
    );

    let ProfileAgeCredentialReadOutcome::Observed { current, .. } =
        ProfileStore::profile_age_credential(&store, read_request(&profile_id))
            .await
            .expect("read pre-import credential state")
    else {
        panic!("expected current credential state")
    };
    assert_eq!(
        current,
        observation(&profile_id, 0, ProfileAgeCredentialState::NotConfigured)
    );

    let identity = Identity::generate();
    let private_key_text = identity.to_string().expose_secret().to_owned();
    let encrypted = age::encrypt_and_armor(&identity.to_public(), b"proxies: []\n")
        .expect("armored Age document");
    ProfileStore::mutate_profile_age_credential(
        &store,
        write_request(
            current,
            ProfileAgeCredentialTarget::Set(AgePrivateKey::new(private_key_text).unwrap()),
        ),
    )
    .await
    .expect("preconfigure Age key for import");

    let decrypted = store
        .decrypt_age_subscription_content(
            &profile_id,
            ProfileAgeEncryptedContent::new(encrypted).unwrap(),
        )
        .await
        .expect("decrypt before profile document exists");
    let ProfileAgeContentDecryption::Decrypted { content, .. } = decrypted else {
        panic!("expected decrypted subscription content")
    };
    ProfileStore::save_inactive_profile(&store, profile_id.as_str(), content.as_str())
        .await
        .expect("create profile after decrypting subscription");
    assert_eq!(
        store
            .read_profile_document(&profile_id)
            .await
            .expect("imported profile document"),
        "proxies: []\n"
    );
}

#[tokio::test]
async fn sing_box_age_subscription_payload_decrypts_inside_its_profile_owner() {
    let root = TempDir::new().expect("config root");
    let engine_id = EngineId::sing_box();
    let store = make_store_for(
        root.path().to_path_buf(),
        engine_id.clone(),
        MemorySecureStore::healthy(),
    );
    let profile_id = ProfileId::new("primary").unwrap();
    create_profile(&store, &profile_id).await;

    let identity = Identity::generate();
    let private_key_text = identity.to_string().expose_secret().to_owned();
    let encrypted = age::encrypt_and_armor(&identity.to_public(), b"outbounds: []\n")
        .expect("armored Age document");
    ProfileStore::mutate_profile_age_credential(
        &store,
        write_request(
            observation_for(
                engine_id,
                &profile_id,
                0,
                ProfileAgeCredentialState::NotConfigured,
            ),
            ProfileAgeCredentialTarget::Set(AgePrivateKey::new(private_key_text).unwrap()),
        ),
    )
    .await
    .expect("configure sing-box Age key");

    let decrypted = store
        .decrypt_age_subscription_content(
            &profile_id,
            ProfileAgeEncryptedContent::new(encrypted).unwrap(),
        )
        .await
        .expect("decrypt inside sing-box profile owner");
    let ProfileAgeContentDecryption::Decrypted {
        content,
        credential_revision,
    } = decrypted
    else {
        panic!("expected decrypted profile content")
    };
    assert_eq!(content.as_str(), "outbounds: []\n");
    assert_eq!(credential_revision, ProfileAgeCredentialRevision::new(1));
}

#[tokio::test]
async fn deleting_profile_retires_age_secret_without_resetting_credential_revision() {
    let root = TempDir::new().expect("config root");
    let secure_store = MemorySecureStore::healthy();
    let secure_store_probe = MemorySecureStore {
        values: Arc::clone(&secure_store.values),
        mode: FakeSecureStoreMode::Healthy,
    };
    let store = make_store(root.path().to_path_buf(), secure_store);
    let profile_id = ProfileId::new("primary").unwrap();
    create_profile(&store, &profile_id).await;
    let secret = "AGE-SECRET-KEY-delete-on-profile-removal";

    let result = ProfileStore::mutate_profile_age_credential(
        &store,
        write_request(
            observation(&profile_id, 0, ProfileAgeCredentialState::NotConfigured),
            ProfileAgeCredentialTarget::Set(AgePrivateKey::new(secret.to_owned()).unwrap()),
        ),
    )
    .await
    .expect("configure key");
    assert!(matches!(
        result,
        ProfileAgeCredentialMutation::Updated { .. }
    ));
    assert!(secure_store_probe.contains_value(secret));

    ProfileStore::delete_profile(&store, profile_id.as_str())
        .await
        .expect("delete profile");

    let retired = read_credential_record(&store, &profile_id)
        .await
        .expect("credential tombstone");
    assert_eq!(
        retired.observation(EngineId::mihomo(), profile_id.clone()),
        observation(&profile_id, 2, ProfileAgeCredentialState::NotConfigured)
    );
    assert!(!secure_store_probe.contains_value(secret));
    assert!(
        store
            .existing_profile_document_path(&profile_id)
            .await
            .is_err()
    );
}
