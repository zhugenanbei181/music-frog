//! test-intent: behavior

use async_trait::async_trait;
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReferenceCasOutcome, ActiveProfileReferenceState, ActiveProfileReferenceTarget,
    ActiveProfileReferenceWriteAuthorization,
};
use infiltrator_contract::core_identity::EngineId;
use infiltrator_contract::error::ErrorCode;
use infiltrator_contract::profile_document_identity::ProfileId;
use infiltrator_domain::profile_options::options_path;
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::profile_workspace::{ProfileWorkspacePurpose, ProfileWorkspaceUpdate};
use infiltrator_ports::secure_store::SecureStore;
use mihomo_config::manager::{ConfigManager, ProfileDocumentExtension};
use singbox_config::profile_document_codec::{PROFILE_FORMAT, SingBoxProfileDocumentCodec};
use std::str::from_utf8;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::fs::{create_dir_all, read, write};

struct TestCredentialStore;

#[async_trait]
impl SecureStore for TestCredentialStore {
    async fn get(&self, _: &str, _: &str) -> Result<Option<String>, PortError> {
        Ok(None)
    }

    async fn set(&self, _: &str, _: &str, _: &str) -> Result<(), PortError> {
        Ok(())
    }

    async fn delete(&self, _: &str, _: &str) -> Result<(), PortError> {
        Ok(())
    }
}

fn mihomo_store(home: &TempDir) -> ConfigManager<TestCredentialStore> {
    ConfigManager::with_home_and_store(home.path().to_path_buf(), TestCredentialStore)
        .expect("Mihomo profile store")
}

fn singbox_store(home: &TempDir) -> ConfigManager<TestCredentialStore> {
    ConfigManager::with_profile_codec_and_store(
        home.path().to_path_buf(),
        None,
        TestCredentialStore,
        Arc::new(SingBoxProfileDocumentCodec),
        ProfileDocumentExtension::Json,
    )
    .expect("sing-box profile store")
}

#[test]
fn engine_store_can_reuse_an_exact_shared_configs_root_path() {
    let home = TempDir::new().unwrap();
    let configs_root = home.path().join("shared configs");
    let singbox = ConfigManager::with_profile_codec_and_store_at_root(
        home.path().to_path_buf(),
        configs_root.clone(),
        TestCredentialStore,
        Arc::new(SingBoxProfileDocumentCodec),
        ProfileDocumentExtension::Json,
    )
    .expect("construct sing-box manager using the resolved root");

    assert_eq!(singbox.config_root(), configs_root);
    assert_eq!(
        singbox.config_dir(),
        singbox.config_root().join("engines").join("sing-box")
    );
}

#[tokio::test]
async fn engine_profile_stores_share_one_active_reference_and_isolate_native_documents() {
    let home = TempDir::new().unwrap();
    let singbox = singbox_store(&home);
    let legacy_document = singbox.config_root().join("legacy.yaml");
    let legacy_settings = home.path().join("config.toml");
    create_dir_all(singbox.config_root()).await.unwrap();
    write(&legacy_document, b"mode: rule\n").await.unwrap();
    write(&legacy_settings, b"[default]\nprofile = \"legacy\"\n")
        .await
        .unwrap();

    singbox
        .initialize_profile_namespace()
        .await
        .expect("initialize engine namespace without importing Mihomo legacy state");
    assert!(
        legacy_document.exists(),
        "foreign profile bytes stay untouched"
    );
    assert!(legacy_settings.exists(), "Mihomo settings stay untouched");
    assert!(singbox.config_dir().join("profile-state.toml").exists());

    let singbox_document = r#"{"outbounds":[],"route":{}}"#;
    ProfileStore::save_inactive_profile(&singbox, "main", singbox_document)
        .await
        .expect("save codec-validated sing-box JSON");
    let singbox_workspace =
        ProfileStore::load_workspace(&singbox, &ProfileId::new("main").expect("profile ID"))
            .await
            .expect("read sing-box workspace");
    assert_eq!(
        singbox_workspace.document.identity.engine_id,
        EngineId::new("sing-box").unwrap()
    );
    assert_eq!(
        singbox_workspace.document.identity.format_id.as_str(),
        PROFILE_FORMAT
    );
    assert_eq!(
        singbox_workspace.document.raw_bytes,
        singbox_document.as_bytes()
    );
    assert_eq!(
        read(singbox.config_dir().join("main.json")).await.unwrap(),
        singbox_document.as_bytes()
    );
    assert!(!singbox.config_dir().join("main.yaml").exists());

    let expected = ProfileStore::read_active_profile_reference(&singbox)
        .await
        .expect("read product-wide reference");
    let selected = match ProfileStore::compare_and_set_active_profile_reference(
        &singbox,
        expected.clone(),
        ActiveProfileReferenceTarget::selected(
            singbox.engine_id(),
            ProfileId::new("main").expect("profile ID"),
        ),
        ActiveProfileReferenceWriteAuthorization::Unowned,
    )
    .await
    .expect("publish sing-box selection through the global CAS")
    {
        ActiveProfileReferenceCasOutcome::Committed { current, .. } => current,
        ActiveProfileReferenceCasOutcome::Stale { .. } => {
            panic!("isolated sing-box selection unexpectedly went stale")
        }
    };
    assert_eq!(
        ProfileStore::get_current(&singbox)
            .await
            .expect("sing-box sees its selected profile"),
        "main"
    );

    let mihomo = mihomo_store(&home);
    assert_eq!(
        ProfileStore::read_active_profile_reference(&mihomo)
            .await
            .expect("Mihomo observes the same product-wide reference"),
        selected
    );
    assert!(matches!(
        ProfileStore::get_current(&mihomo).await,
        Err(PortError::Unsupported { .. })
    ));
    ProfileStore::save_inactive_profile(&mihomo, "main", "mode: rule\n")
        .await
        .expect("same profile name has an independent Mihomo namespace");
    assert_eq!(
        read(mihomo.config_dir().join("main.yaml")).await.unwrap(),
        b"mode: rule\n"
    );
    assert_eq!(
        read(singbox.config_dir().join("main.json")).await.unwrap(),
        singbox_document.as_bytes()
    );
    assert!(matches!(selected, ActiveProfileReferenceState::Selected(_)));
}

#[tokio::test]
async fn singbox_store_rejects_invalid_json_before_document_publication() {
    let home = TempDir::new().unwrap();
    let singbox = singbox_store(&home);

    let result =
        ProfileStore::save_inactive_profile(&singbox, "invalid", "{\"outbounds\":[}").await;
    assert!(matches!(
        result,
        Err(PortError::Rejected(failure)) if failure.code == ErrorCode::Configuration
    ));
    assert!(!singbox.config_dir().join("invalid.json").exists());
}

#[tokio::test]
async fn singbox_workspace_round_trips_exact_sidecar_and_fences_stale_or_foreign_writes() {
    let home = TempDir::new().unwrap();
    let singbox = singbox_store(&home);
    let profile = ProfileId::new("main").unwrap();
    let initial_document = br#"{"outbounds":[{"type":"direct","tag":"direct"}]}"#;
    ProfileStore::save_inactive_profile(&singbox, "main", from_utf8(initial_document).unwrap())
        .await
        .unwrap();

    let sidecar_path = options_path(&singbox.config_dir(), "main");
    create_dir_all(sidecar_path.parent().unwrap())
        .await
        .unwrap();
    let original_sidecar = b"\x00opaque\xffsidecar\r\n";
    write(&sidecar_path, original_sidecar).await.unwrap();
    let observed = ProfileStore::load_workspace(&singbox, &profile)
        .await
        .unwrap();
    assert_eq!(observed.document.raw_bytes, initial_document);
    assert_eq!(
        observed.sidecar_bytes.as_deref(),
        Some(original_sidecar.as_slice())
    );

    let next_document = br#"{"outbounds":[{"type":"block","tag":"blocked"}]}"#;
    let next_sidecar = b"\xffnew\x00opaque";
    let update = ProfileWorkspaceUpdate {
        purpose: ProfileWorkspacePurpose::Derived,
        document_bytes: next_document.to_vec(),
        sidecar_bytes: Some(next_sidecar.to_vec()),
    };
    let committed =
        ProfileStore::compare_and_save_inactive_workspace(&singbox, &observed.identity(), &update)
            .await
            .unwrap();
    assert_eq!(committed.document.raw_bytes, next_document);
    assert_eq!(
        committed.sidecar_bytes.as_deref(),
        Some(next_sidecar.as_slice())
    );
    assert_eq!(
        read(singbox.config_dir().join("main.json")).await.unwrap(),
        next_document
    );
    assert_eq!(read(&sidecar_path).await.unwrap(), next_sidecar);

    let before_stale = ProfileStore::load_workspace(&singbox, &profile)
        .await
        .unwrap();
    let stale = ProfileStore::compare_and_save_inactive_workspace(
        &singbox,
        &observed.identity(),
        &ProfileWorkspaceUpdate {
            purpose: ProfileWorkspacePurpose::Derived,
            document_bytes: br#"{"outbounds":[]}"#.to_vec(),
            sidecar_bytes: None,
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(stale, PortError::Rejected(failure) if failure.code == ErrorCode::NotReady));
    assert_eq!(
        ProfileStore::load_workspace(&singbox, &profile)
            .await
            .unwrap(),
        before_stale
    );

    let mut foreign = before_stale.identity();
    foreign.document.engine_id = EngineId::new("mihomo").unwrap();
    let foreign_write = ProfileStore::compare_and_save_inactive_workspace(
        &singbox,
        &foreign,
        &ProfileWorkspaceUpdate {
            purpose: ProfileWorkspacePurpose::Derived,
            document_bytes: br#"{"outbounds":[]}"#.to_vec(),
            sidecar_bytes: None,
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(foreign_write, PortError::Unsupported { .. }));
    assert_eq!(
        ProfileStore::load_workspace(&singbox, &profile)
            .await
            .unwrap(),
        before_stale
    );

    let expected_reference = ProfileStore::read_active_profile_reference(&singbox)
        .await
        .unwrap();
    let selected = match ProfileStore::compare_and_set_active_profile_reference(
        &singbox,
        expected_reference,
        ActiveProfileReferenceTarget::selected(singbox.engine_id(), profile.clone()),
        ActiveProfileReferenceWriteAuthorization::Unowned,
    )
    .await
    .unwrap()
    {
        ActiveProfileReferenceCasOutcome::Committed { current, .. } => current,
        ActiveProfileReferenceCasOutcome::Stale { .. } => {
            panic!("sing-box selection unexpectedly went stale")
        }
    };
    let reference = match selected {
        ActiveProfileReferenceState::Selected(reference) => reference,
        ActiveProfileReferenceState::Unselected { .. } => {
            panic!("sing-box profile selection was not published")
        }
    };
    let active_workspace = ProfileStore::load_active_workspace(&singbox).await.unwrap();
    let lease = ProfileStore::acquire_active_workspace_write_lease(
        &singbox,
        reference,
        active_workspace.identity(),
        ActiveProfileReferenceWriteAuthorization::Unowned,
    )
    .await
    .unwrap();
    assert_eq!(
        lease.observed_workspace().document.identity.engine_id,
        singbox.engine_id()
    );
    drop(lease);
}
