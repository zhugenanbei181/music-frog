use crate::document_store::write_document_bytes_durable;
use crate::file_lock::lock_profile_writes_for_dir;
use crate::store::{EngineProfileStore, ProfileDocumentExtension};
use crate::workspace_store::ProfileWorkspacePublicationTarget;
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
use infiltrator_ports::profile_workspace::{ProfileWorkspacePurpose, ProfileWorkspaceUpdate};
use infiltrator_ports::secure_store::SecureStore;
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

fn store(root: PathBuf) -> EngineProfileStore<NoopSecureStore> {
    let engine_id = EngineId::new("fixture-engine").unwrap();
    let codec = Arc::new(TestCodec(ProfileDocumentCodecScope {
        engine_id: engine_id.clone(),
        format_id: ProfileFormatId::new("fixture-native").unwrap(),
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
async fn shared_workspace_publication_cas_writes_exact_document_and_sidecar_bytes() {
    let root = TempDir::new().unwrap();
    let store = store(root.path().to_path_buf());
    let profile = ProfileId::new("main").unwrap();
    store
        .write_profile_document_locked(&profile, br#"{"revision":1}"#)
        .await
        .unwrap();
    let sidecar_path = options_path(store.config_dir(), profile.as_str());
    write_document_bytes_durable(&sidecar_path, b"sidecar-v1")
        .await
        .unwrap();
    let previous = store.read_profile_workspace(&profile).await.unwrap();
    let expected = previous.identity();
    let update = ProfileWorkspaceUpdate {
        purpose: ProfileWorkspacePurpose::Derived,
        document_bytes: br#"{"revision":2}"#.to_vec(),
        sidecar_bytes: Some(b"sidecar-v2".to_vec()),
    };

    let guard = lock_profile_writes_for_dir(store.config_dir())
        .await
        .unwrap();
    let published = store
        .publish_profile_workspace_locked(
            &expected,
            &update,
            ProfileWorkspacePublicationTarget::AuthorizedActive,
        )
        .await
        .unwrap();
    drop(guard);

    assert_eq!(published.document.raw_bytes, update.document_bytes);
    assert_eq!(published.sidecar_bytes, update.sidecar_bytes);
    assert_eq!(
        store.read_profile_workspace(&profile).await.unwrap(),
        published
    );
}

#[tokio::test]
async fn shared_workspace_publication_rejects_stale_identity_without_writes() {
    let root = TempDir::new().unwrap();
    let store = store(root.path().to_path_buf());
    let profile = ProfileId::new("main").unwrap();
    store
        .write_profile_document_locked(&profile, br#"{"revision":1}"#)
        .await
        .unwrap();
    let sidecar_path = options_path(store.config_dir(), profile.as_str());
    write_document_bytes_durable(&sidecar_path, b"sidecar-v1")
        .await
        .unwrap();
    let stale = store.read_profile_workspace(&profile).await.unwrap();
    let stale_identity = stale.identity();
    let update = ProfileWorkspaceUpdate {
        purpose: ProfileWorkspacePurpose::Derived,
        document_bytes: br#"{"revision":2}"#.to_vec(),
        sidecar_bytes: Some(b"sidecar-v2".to_vec()),
    };

    let guard = lock_profile_writes_for_dir(store.config_dir())
        .await
        .unwrap();
    store
        .publish_profile_workspace_locked(
            &stale_identity,
            &update,
            ProfileWorkspacePublicationTarget::AuthorizedActive,
        )
        .await
        .unwrap();
    let error = store
        .publish_profile_workspace_locked(
            &stale_identity,
            &ProfileWorkspaceUpdate {
                purpose: ProfileWorkspacePurpose::Derived,
                document_bytes: br#"{"revision":3}"#.to_vec(),
                sidecar_bytes: Some(b"sidecar-v3".to_vec()),
            },
            ProfileWorkspacePublicationTarget::AuthorizedActive,
        )
        .await
        .unwrap_err();
    drop(guard);

    assert!(
        matches!(error, PortError::Rejected(failure) if failure.code == infiltrator_contract::error::ErrorCode::NotReady)
    );
    let observed = store.read_profile_workspace(&profile).await.unwrap();
    assert_eq!(observed.document.raw_bytes, br#"{"revision":2}"#);
    assert_eq!(
        observed.sidecar_bytes.as_deref(),
        Some(b"sidecar-v2".as_slice())
    );
}
