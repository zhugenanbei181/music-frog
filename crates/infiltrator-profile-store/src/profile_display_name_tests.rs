use crate::store::{EngineProfileStore, ProfileDocumentExtension};
use infiltrator_contract::profile_display_name::{
    ProfileDisplayName, ProfileDisplayNameMutation, ProfileDisplayNameObservation,
    ProfileDisplayNameTarget,
};
use infiltrator_contract::profile_document::DocumentValidation;
use infiltrator_contract::profile_document_identity::ProfileId;
use infiltrator_contract::profile_document_identity::{ProfileFormatId, ProfileSchemaVersion};
use infiltrator_domain::profiles::ProfileMetadata;
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_document_codec::{
    ProfileDocumentCodecPort, ProfileDocumentCodecScope,
};
use infiltrator_ports::secure_store::SecureStore;
use lurker_core_contract::identity::EngineId;
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

fn make_store(root: PathBuf) -> EngineProfileStore<NoopSecureStore> {
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
        NoopSecureStore,
    )
    .unwrap()
}

#[tokio::test]
async fn display_name_update_reads_back_and_reset_preserves_profile_identity() {
    let root = TempDir::new().unwrap();
    let store = make_store(root.path().to_path_buf());
    let profile = ProfileId::new("primary").unwrap();
    store
        .write_profile_document_locked(&profile, b"profile bytes")
        .await
        .unwrap();
    store
        .update_profile_metadata(
            "primary",
            &ProfileMetadata {
                user_agent: Some("client/1".to_owned()),
                ..Default::default()
            },
        )
        .await
        .unwrap();

    let label = ProfileDisplayName::new("Work profile").unwrap();
    assert_eq!(
        store
            .set_profile_display_name(&profile, ProfileDisplayNameTarget::Custom(label.clone()),)
            .await
            .unwrap(),
        ProfileDisplayNameMutation::Applied {
            previous: ProfileDisplayNameObservation::Identity,
            current: ProfileDisplayNameObservation::Custom(label.clone()),
        }
    );
    assert_eq!(
        store.profile_display_name(&profile).await.unwrap(),
        ProfileDisplayNameObservation::Custom(label.clone())
    );
    store
        .update_profile_metadata(
            "primary",
            &ProfileMetadata {
                user_agent: Some("client/2".to_owned()),
                auto_update_enabled: true,
                update_interval_hours: Some(12),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(
        store.profile_display_name(&profile).await.unwrap(),
        ProfileDisplayNameObservation::Custom(label.clone())
    );
    assert_eq!(
        store
            .set_profile_display_name(&profile, ProfileDisplayNameTarget::Custom(label.clone()),)
            .await
            .unwrap(),
        ProfileDisplayNameMutation::Unchanged {
            current: ProfileDisplayNameObservation::Custom(label),
        }
    );

    let reopened = make_store(root.path().to_path_buf());
    assert_eq!(
        reopened
            .profile_display_name(&profile)
            .await
            .expect("display name persisted across store recreation"),
        ProfileDisplayNameObservation::Custom(ProfileDisplayName::new("Work profile").unwrap())
    );
    assert_eq!(
        reopened
            .get_profile_metadata("primary")
            .await
            .unwrap()
            .user_agent
            .as_deref(),
        Some("client/2")
    );
    let metadata = reopened.get_profile_metadata("primary").await.unwrap();
    assert!(metadata.auto_update_enabled);
    assert_eq!(metadata.update_interval_hours, Some(12));

    assert_eq!(
        reopened
            .set_profile_display_name(&profile, ProfileDisplayNameTarget::Identity)
            .await
            .unwrap(),
        ProfileDisplayNameMutation::Applied {
            previous: ProfileDisplayNameObservation::Custom(
                ProfileDisplayName::new("Work profile").unwrap(),
            ),
            current: ProfileDisplayNameObservation::Identity,
        }
    );
    let settings = reopened.read_settings_value().await.unwrap();
    assert!(
        !settings["profiles"]["primary"]
            .as_table()
            .unwrap()
            .contains_key("display_name")
    );
    assert_eq!(
        fs::read(reopened.config_dir().join("primary.json"))
            .await
            .unwrap(),
        b"profile bytes"
    );
}

#[tokio::test]
async fn corrupt_labels_fail_closed_without_mutating_settings() {
    let root = TempDir::new().unwrap();
    let store = make_store(root.path().to_path_buf());
    store
        .write_profile_document_locked(&ProfileId::new("primary").unwrap(), b"profile")
        .await
        .unwrap();
    store
        .update_profile_metadata("primary", &ProfileMetadata::default())
        .await
        .unwrap();
    let profile = ProfileId::new("primary").unwrap();
    assert_eq!(
        store.profile_display_name(&profile).await.unwrap(),
        ProfileDisplayNameObservation::Identity
    );
    let mut corrupt = store.read_settings_value().await.unwrap();
    let profiles = corrupt
        .as_table_mut()
        .unwrap()
        .entry("profiles".to_owned())
        .or_insert_with(|| toml::Value::Table(Default::default()))
        .as_table_mut()
        .unwrap();
    let profile_row = profiles
        .entry("primary".to_owned())
        .or_insert_with(|| toml::Value::Table(Default::default()))
        .as_table_mut()
        .unwrap();
    profile_row.insert("display_name".to_owned(), toml::Value::Integer(7));
    store.write_settings_value(&corrupt).await.unwrap();
    let before_read = store.read_settings_value().await.unwrap();
    assert!(store.profile_display_name(&profile).await.is_err());
    assert_eq!(store.read_settings_value().await.unwrap(), before_read);
}
