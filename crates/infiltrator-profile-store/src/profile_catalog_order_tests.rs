use crate::store::{EngineProfileStore, ProfileDocumentExtension};
use infiltrator_contract::profile_catalog_order::{
    ProfileCatalogOrderMutation, ProfileCatalogOrderRequest, ProfileCatalogSnapshot,
};
use infiltrator_contract::profile_document_identity::ProfileId;
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::secure_store::SecureStore;
use lurker_core_contract::identity::EngineId;
use lurker_core_contract::profile_document::DocumentValidation;
use lurker_core_contract::profile_document_identity::{ProfileFormatId, ProfileSchemaVersion};
use lurker_core_ports::error::PortError;
use lurker_core_ports::profile_document_codec::{
    ProfileDocumentCodecPort, ProfileDocumentCodecScope,
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
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

#[derive(Default)]
struct MemorySecureStore {
    values: Mutex<HashMap<(String, String), String>>,
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

fn make_store(root: PathBuf) -> EngineProfileStore<MemorySecureStore> {
    let engine_id = EngineId::new("mihomo").unwrap();
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
        MemorySecureStore::default(),
    )
    .unwrap()
}

async fn write_profiles(store: &EngineProfileStore<MemorySecureStore>, names: &[&str]) {
    for name in names {
        store
            .write_profile_document_locked(&ProfileId::new(*name).unwrap(), b"profile")
            .await
            .unwrap();
    }
}

async fn snapshot(store: &EngineProfileStore<MemorySecureStore>) -> ProfileCatalogSnapshot {
    let profiles = ProfileStore::list_profiles(store).await.unwrap();
    ProfileCatalogSnapshot::new(
        EngineId::new("mihomo").unwrap(),
        profiles
            .into_iter()
            .map(|profile| ProfileId::new(profile.name).unwrap())
            .collect(),
    )
}

#[tokio::test]
async fn catalog_order_persists_exact_identity_order_and_legacy_order_is_alphabetical() {
    let root = TempDir::new().unwrap();
    let store = make_store(root.path().to_path_buf());
    write_profiles(&store, &["gamma", "alpha", "beta"]).await;

    let expected = snapshot(&store).await;
    assert_eq!(
        expected.ordered_profile_ids,
        ["alpha", "beta", "gamma"].map(|name| ProfileId::new(name).unwrap())
    );
    let target_order = ["gamma", "alpha", "beta"].map(|name| ProfileId::new(name).unwrap());
    assert_eq!(
        ProfileStore::set_profile_catalog_order(
            &store,
            ProfileCatalogOrderRequest::new(expected.clone(), target_order.to_vec()),
        )
        .await
        .unwrap(),
        ProfileCatalogOrderMutation::Applied {
            previous: expected,
            current: ProfileCatalogSnapshot::new(
                EngineId::new("mihomo").unwrap(),
                target_order.to_vec(),
            ),
        }
    );

    let reopened = make_store(root.path().to_path_buf());
    assert_eq!(snapshot(&reopened).await.ordered_profile_ids, target_order);
    ProfileStore::save_inactive_profile(&reopened, "delta", "profile")
        .await
        .unwrap();
    assert_eq!(
        snapshot(&reopened).await.ordered_profile_ids,
        vec![
            ProfileId::new("gamma").unwrap(),
            ProfileId::new("alpha").unwrap(),
            ProfileId::new("beta").unwrap(),
            ProfileId::new("delta").unwrap(),
        ]
    );
}

#[tokio::test]
async fn invalid_or_stale_order_never_replaces_the_current_catalog() {
    let root = TempDir::new().unwrap();
    let store = make_store(root.path().to_path_buf());
    write_profiles(&store, &["one", "two", "three"]).await;
    let expected = snapshot(&store).await;

    for target_order in [
        vec![
            ProfileId::new("one").unwrap(),
            ProfileId::new("one").unwrap(),
            ProfileId::new("three").unwrap(),
        ],
        vec![
            ProfileId::new("one").unwrap(),
            ProfileId::new("three").unwrap(),
        ],
        vec![
            ProfileId::new("one").unwrap(),
            ProfileId::new("three").unwrap(),
            ProfileId::new("foreign").unwrap(),
        ],
    ] {
        assert!(
            ProfileStore::set_profile_catalog_order(
                &store,
                ProfileCatalogOrderRequest::new(expected.clone(), target_order),
            )
            .await
            .is_err()
        );
        assert_eq!(snapshot(&store).await, expected);
    }

    ProfileStore::delete_profile(&store, "two").await.unwrap();
    let target_order = vec![
        ProfileId::new("three").unwrap(),
        ProfileId::new("one").unwrap(),
        ProfileId::new("two").unwrap(),
    ];
    assert_eq!(
        ProfileStore::set_profile_catalog_order(
            &store,
            ProfileCatalogOrderRequest::new(expected.clone(), target_order),
        )
        .await
        .unwrap(),
        ProfileCatalogOrderMutation::Stale {
            expected,
            observed: ProfileCatalogSnapshot::new(
                EngineId::new("mihomo").unwrap(),
                vec![
                    ProfileId::new("one").unwrap(),
                    ProfileId::new("three").unwrap()
                ],
            ),
        }
    );
}

#[tokio::test]
async fn concurrent_catalog_reorder_and_profile_delete_leave_a_complete_order() {
    let root = TempDir::new().unwrap();
    let store = make_store(root.path().to_path_buf());
    write_profiles(&store, &["alpha", "beta", "gamma"]).await;
    let expected = snapshot(&store).await;
    let target = ["gamma", "beta", "alpha"]
        .map(|name| ProfileId::new(name).unwrap())
        .to_vec();
    let reorder = ProfileStore::set_profile_catalog_order(
        &store,
        ProfileCatalogOrderRequest::new(expected.clone(), target.clone()),
    );
    let delete = ProfileStore::delete_profile(&store, "beta");
    let (reorder, delete) = tokio::join!(reorder, delete);
    delete.unwrap();

    let current = snapshot(&store).await;
    match reorder.unwrap() {
        ProfileCatalogOrderMutation::Applied {
            previous,
            current: applied,
        } => {
            assert_eq!(previous, expected);
            assert_eq!(applied.ordered_profile_ids, target);
            assert_eq!(
                current.ordered_profile_ids,
                vec![
                    ProfileId::new("gamma").unwrap(),
                    ProfileId::new("alpha").unwrap()
                ]
            );
        }
        ProfileCatalogOrderMutation::Stale {
            expected: observed_expected,
            observed,
        } => {
            assert_eq!(observed_expected, expected);
            assert_eq!(observed, current);
            assert_eq!(
                current.ordered_profile_ids,
                vec![
                    ProfileId::new("alpha").unwrap(),
                    ProfileId::new("gamma").unwrap()
                ]
            );
        }
        ProfileCatalogOrderMutation::Unchanged { .. } => {
            panic!("a changed target order cannot be unchanged")
        }
    }
}
