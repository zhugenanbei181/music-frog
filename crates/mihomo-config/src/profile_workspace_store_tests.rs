//! test-intent: behavior
use super::*;
use crate::profile_document_codec::{PROFILE_FORMAT, PROFILE_SCHEMA_VERSION};
use async_trait::async_trait;
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReferenceCasOutcome, ActiveProfileReferenceState, ActiveProfileReferenceTarget,
    ActiveProfileReferenceWriteAuthorization,
};
use infiltrator_contract::core_identity::{CoreGeneration, CoreIdentity, CoreInstanceId, EngineId};
use infiltrator_contract::error::Failure;
use infiltrator_contract::profile_document_identity::{
    ProfileDocument, ProfileDocumentIdentity, ProfileDocumentSourceIdentity,
    ProfileDocumentSourceKind, ProfileFormatId, ProfileId, ProfileSchemaVersion,
    ProfileSidecarRevision, ProfileSourceId, ProfileWorkspaceIdentity,
};
use infiltrator_contract::profile_protection::{ProfileWriteAuthorization, ProfileWriteProtection};
use infiltrator_contract::snapshot::{CoreLifecycle, CoreLifecycleSnapshot};
use infiltrator_domain::profile_options::options_path;
use infiltrator_ports::active_profile_reference_store::ActiveProfileReferenceStore;
use infiltrator_ports::profile_store::{
    ActiveProfileWorkspacePublishOutcome, ActiveProfileWorkspacePublishedLease,
    ActiveProfileWorkspaceRecoveryResolution, ActiveProfileWorkspaceRestoreOutcome,
    ActiveProfileWorkspaceWriteLease, ProfileStore,
};
use infiltrator_ports::profile_workspace::{
    ProfileWorkspace, ProfileWorkspacePurpose, ProfileWorkspaceUpdate,
};
use infiltrator_ports::secure_store::SecureStore;
use infiltrator_profile_store::document_store::{
    read_optional_bytes, write_document, write_document_bytes,
};
use infiltrator_profile_store::profile_reset::{
    ActiveProfileReferenceResetOutcome, ActiveProfileReferenceResetRequest,
};
use infiltrator_profile_store::workspace::{document_revision, publish_document_pair, sha256_hex};
use std::collections::HashMap;
use std::str;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tempfile::TempDir;
use tokio::fs::{create_dir_all, read, remove_file, write};
use tokio::time::timeout;

#[derive(Default)]
struct TestStore(Mutex<HashMap<(String, String), String>>);

#[async_trait]
impl SecureStore for TestStore {
    async fn get(&self, namespace: &str, key: &str) -> Result<Option<String>, PortError> {
        if key.starts_with("subscription:") {
            return Ok(None);
        }
        Ok(self
            .0
            .lock()
            .unwrap()
            .get(&(namespace.to_owned(), key.to_owned()))
            .cloned())
    }

    async fn set(&self, namespace: &str, key: &str, value: &str) -> Result<(), PortError> {
        if key.starts_with("subscription:") {
            return Ok(());
        }
        self.0
            .lock()
            .unwrap()
            .insert((namespace.to_owned(), key.to_owned()), value.to_owned());
        Ok(())
    }

    async fn delete(&self, namespace: &str, key: &str) -> Result<(), PortError> {
        self.0
            .lock()
            .unwrap()
            .remove(&(namespace.to_owned(), key.to_owned()));
        Ok(())
    }
}

fn manager(dir: &TempDir) -> ConfigManager<TestStore> {
    ConfigManager::with_home_and_store(dir.path().to_path_buf(), TestStore::default()).unwrap()
}

async fn select_profile<S: SecureStore>(manager: &ConfigManager<S>, profile: &str) {
    let expected = ProfileStore::read_active_profile_reference(manager.engine_profile_store())
        .await
        .expect("read selection before test CAS");
    let target = ActiveProfileReferenceTarget::selected(
        ConfigManager::engine_id(manager),
        ProfileId::new(profile).expect("valid test profile ID"),
    );
    match ProfileStore::compare_and_set_active_profile_reference(
        manager.engine_profile_store(),
        expected,
        target,
        ActiveProfileReferenceWriteAuthorization::Unowned,
    )
    .await
    .expect("revisioned test selection CAS")
    {
        ActiveProfileReferenceCasOutcome::Committed { .. } => {}
        ActiveProfileReferenceCasOutcome::Stale { expected, observed } => {
            panic!("test selection unexpectedly stale: {expected:?} -> {observed:?}")
        }
    }
}

fn profile_id() -> ProfileId {
    ProfileId::new("main").unwrap()
}

fn update(document: &[u8], sidecar: Option<&[u8]>) -> ProfileWorkspaceUpdate {
    ProfileWorkspaceUpdate {
        purpose: ProfileWorkspacePurpose::Derived,
        document_bytes: document.to_vec(),
        sidecar_bytes: sidecar.map(<[u8]>::to_vec),
    }
}

async fn publish_or_panic(
    observed: Box<dyn ActiveProfileWorkspaceWriteLease + '_>,
    update: ProfileWorkspaceUpdate,
) -> Box<dyn ActiveProfileWorkspacePublishedLease + '_> {
    match observed.publish(update).await {
        ActiveProfileWorkspacePublishOutcome::Published(published) => published,
        ActiveProfileWorkspacePublishOutcome::Unpublished(error) => {
            panic!("active workspace was not published: {error}")
        }
        ActiveProfileWorkspacePublishOutcome::Superseded { observed } => {
            panic!("active workspace was superseded: {observed:?}")
        }
        ActiveProfileWorkspacePublishOutcome::RecoveryRequired(mut recovery) => loop {
            match recovery.resolve().await {
                ActiveProfileWorkspaceRecoveryResolution::Published(published) => break published,
                ActiveProfileWorkspaceRecoveryResolution::Restored(restored) => {
                    let _ = restored;
                    panic!("active workspace was restored instead of published")
                }
                ActiveProfileWorkspaceRecoveryResolution::Unpublished(error) => {
                    panic!("active workspace publication rolled back: {error}")
                }
                ActiveProfileWorkspaceRecoveryResolution::Superseded { observed } => {
                    panic!("active workspace recovery was superseded: {observed:?}")
                }
                ActiveProfileWorkspaceRecoveryResolution::RecoveryRequired(next) => {
                    recovery = next;
                }
            }
        },
    }
}

fn direct_update(
    document: &[u8],
    sidecar: Option<&[u8]>,
    authorization: ProfileWriteAuthorization,
) -> ProfileWorkspaceUpdate {
    ProfileWorkspaceUpdate {
        purpose: ProfileWorkspacePurpose::DirectEdit { authorization },
        document_bytes: document.to_vec(),
        sidecar_bytes: sidecar.map(<[u8]>::to_vec),
    }
}

fn foreign_identity(profile: &ProfileId) -> ProfileWorkspaceIdentity {
    let document = ProfileDocumentIdentity::new(
        profile.clone(),
        EngineId::new("sing-box").unwrap(),
        ProfileDocumentSourceIdentity::new(
            ProfileDocumentSourceKind::Local,
            ProfileSourceId::new("foreign-fixture").unwrap(),
        ),
        document_revision(b"mode: rule\n"),
        ProfileFormatId::new(PROFILE_FORMAT).unwrap(),
        ProfileSchemaVersion::new(PROFILE_SCHEMA_VERSION).unwrap(),
    );
    ProfileWorkspaceIdentity::new(document, ProfileSidecarRevision::Absent)
}

fn mihomo_identity_with_format(
    profile: &ProfileId,
    format_id: &str,
    schema_version: u32,
) -> ProfileWorkspaceIdentity {
    let document = ProfileDocumentIdentity::new(
        profile.clone(),
        crate::engine_id(),
        ProfileDocumentSourceIdentity::new(
            ProfileDocumentSourceKind::Local,
            ProfileSourceId::new("mihomo-fixture").unwrap(),
        ),
        document_revision(b"mode: rule\n"),
        ProfileFormatId::new(format_id).unwrap(),
        ProfileSchemaVersion::new(schema_version).unwrap(),
    );
    ProfileWorkspaceIdentity::new(document, ProfileSidecarRevision::Absent)
}

#[test]
fn typed_profile_ids_reject_path_fragments_before_load_boundary() {
    for value in ["../sentinel", r"..\sentinel", ".", ".."] {
        assert!(
            ProfileId::new(value).is_err(),
            "unsafe profile identity crossed typed boundary: {value:?}"
        );
    }
}

#[tokio::test]
async fn workspace_identity_tracks_exact_document_and_sidecar_bytes() {
    let dir = TempDir::new().unwrap();
    let store = manager(&dir);
    assert_eq!(store.engine_id().as_str(), "mihomo");
    store
        .save_inactive_profile("main", "mode: rule\n")
        .await
        .unwrap();
    let original = store
        .engine_profile_store()
        .load_workspace(&profile_id())
        .await
        .unwrap();
    assert_eq!(original.sidecar_revision, ProfileSidecarRevision::Absent);
    assert_eq!(original.sidecar_bytes, None);
    assert_eq!(
        original.document.identity.revision,
        document_revision(b"mode: rule\n")
    );

    let sidecar = options_path(store.config_dir(), "main");
    create_dir_all(sidecar.parent().unwrap()).await.unwrap();
    write(&sidecar, b"").await.unwrap();
    let empty = store
        .engine_profile_store()
        .load_workspace(&profile_id())
        .await
        .unwrap();
    assert_ne!(empty.identity(), original.identity());
    assert_eq!(empty.sidecar_bytes, Some(Vec::new()));
    assert_eq!(
        empty.sidecar_revision,
        ProfileSidecarRevision::Present(document_revision(b""))
    );

    let annotated = b"# exact annotation\r\n{}\r\n";
    write(&sidecar, annotated).await.unwrap();
    let with_comments = store
        .engine_profile_store()
        .load_workspace(&profile_id())
        .await
        .unwrap();
    assert_eq!(with_comments.document, empty.document);
    assert_eq!(
        with_comments.sidecar_bytes.as_deref(),
        Some(annotated.as_slice())
    );
    assert_ne!(with_comments.identity(), empty.identity());

    let edited_document = b"# authored\r\nmode: rule\r\n";
    store
        .save_inactive_profile("main", str::from_utf8(edited_document).unwrap())
        .await
        .unwrap();
    let edited = store
        .engine_profile_store()
        .load_workspace(&profile_id())
        .await
        .unwrap();
    assert_eq!(edited.document.raw_bytes, edited_document);
    assert_ne!(
        edited.document.identity.revision,
        with_comments.document.identity.revision
    );
    assert_eq!(edited.sidecar_bytes.as_deref(), Some(annotated.as_slice()));
}

#[tokio::test]
async fn document_writers_replace_existing_destinations() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("stored-document.json");
    write(&path, b"old bytes").await.unwrap();

    write_document_bytes(&path, b"new bytes").await.unwrap();
    assert_eq!(read(&path).await.unwrap(), b"new bytes");

    write_document(&path, "final text").await.unwrap();
    assert_eq!(read(&path).await.unwrap(), b"final text");
}

#[tokio::test]
async fn stale_document_or_sidecar_rejects_before_either_file_changes() {
    let dir = TempDir::new().unwrap();
    let store = manager(&dir);
    store
        .save_inactive_profile("main", "mode: rule\n")
        .await
        .unwrap();
    let original = store
        .engine_profile_store()
        .load_workspace(&profile_id())
        .await
        .unwrap();
    store
        .save_inactive_profile("main", "# newer\nmode: direct\n")
        .await
        .unwrap();
    let changed = store
        .engine_profile_store()
        .load_workspace(&profile_id())
        .await
        .unwrap();

    let failure = Failure::from(
        store
            .engine_profile_store()
            .compare_and_save_inactive_workspace(
                &original.identity(),
                &update(b"mode: global\n", None),
            )
            .await
            .unwrap_err(),
    );
    assert_eq!(failure.code, ErrorCode::NotReady);
    assert_eq!(
        store
            .engine_profile_store()
            .load_workspace(&profile_id())
            .await
            .unwrap(),
        changed
    );

    let sidecar = options_path(store.config_dir(), "main");
    create_dir_all(sidecar.parent().unwrap()).await.unwrap();
    write(&sidecar, b"filter: [old]\n").await.unwrap();
    let sidecar_changed = store
        .engine_profile_store()
        .load_workspace(&profile_id())
        .await
        .unwrap();
    let profile_path = store.config_dir().join("main.yaml");
    let profile_before = read(&profile_path).await.unwrap();
    let sidecar_before = read(&sidecar).await.unwrap();
    let failure = Failure::from(
        store
            .engine_profile_store()
            .compare_and_save_inactive_workspace(
                &changed.identity(),
                &update(b"mode: global\n", Some(b"filter: [new]\n")),
            )
            .await
            .unwrap_err(),
    );
    assert_eq!(failure.code, ErrorCode::NotReady);
    assert_eq!(read(&profile_path).await.unwrap(), profile_before);
    assert_eq!(read(&sidecar).await.unwrap(), sidecar_before);
    assert_eq!(
        store
            .engine_profile_store()
            .load_workspace(&profile_id())
            .await
            .unwrap(),
        sidecar_changed
    );
}

#[tokio::test]
async fn inactive_workspace_replaces_existing_profile_and_sidecar() {
    let dir = TempDir::new().unwrap();
    let store = manager(&dir);
    store
        .save_inactive_profile("main", "mode: rule\n")
        .await
        .unwrap();
    let sidecar = options_path(store.config_dir(), "main");
    create_dir_all(sidecar.parent().unwrap()).await.unwrap();
    write(&sidecar, b"filter: [old]\n").await.unwrap();
    let previous = store
        .engine_profile_store()
        .load_workspace(&profile_id())
        .await
        .unwrap();

    let committed = store
        .engine_profile_store()
        .compare_and_save_inactive_workspace(
            &previous.identity(),
            &update(b"mode: direct\n", Some(b"filter: [new]\n")),
        )
        .await
        .unwrap();

    assert_eq!(committed.document.raw_bytes, b"mode: direct\n");
    assert_eq!(
        committed.sidecar_bytes.as_deref(),
        Some(b"filter: [new]\n".as_slice())
    );
    assert_eq!(
        read(store.config_dir().join("main.yaml")).await.unwrap(),
        b"mode: direct\n"
    );
    assert_eq!(read(sidecar).await.unwrap(), b"filter: [new]\n");
}

#[tokio::test]
async fn unowned_derived_active_document_write_is_single_file_and_revision_fenced() {
    let dir = TempDir::new().unwrap();
    let store = manager(&dir);
    store
        .save_inactive_profile("main", "mode: rule\n")
        .await
        .unwrap();
    store
        .save_inactive_profile("default", "mode: direct\n")
        .await
        .unwrap();
    select_profile(&store, "main").await;

    let ActiveProfileReferenceState::Selected(reference) =
        ProfileStore::read_active_profile_reference(store.engine_profile_store())
            .await
            .unwrap()
    else {
        panic!("test profile must be selected");
    };
    let observed = store
        .engine_profile_store()
        .load_active_workspace()
        .await
        .unwrap();
    let sidecar_path = options_path(store.config_dir(), "main");
    create_dir_all(sidecar_path.parent().unwrap())
        .await
        .unwrap();
    write(&sidecar_path, b"filter: [keep]\n").await.unwrap();
    let sidecar_observed = store
        .engine_profile_store()
        .load_active_workspace()
        .await
        .unwrap();
    let committed = ProfileStore::compare_and_save_unowned_active_derived_document(
        store.engine_profile_store(),
        reference.clone(),
        sidecar_observed.identity(),
        &update(b"mode: global\n", sidecar_observed.sidecar_bytes.as_deref()),
    )
    .await
    .unwrap();
    assert_eq!(committed.document.raw_bytes, b"mode: global\n");
    assert_eq!(
        committed.sidecar_bytes.as_deref(),
        Some(b"filter: [keep]\n".as_slice())
    );
    assert_eq!(read(&sidecar_path).await.unwrap(), b"filter: [keep]\n");

    let before_stale_attempt = read(store.config_dir().join("main.yaml")).await.unwrap();
    select_profile(&store, "default").await;
    let stale = ProfileStore::compare_and_save_unowned_active_derived_document(
        store.engine_profile_store(),
        reference,
        observed.identity(),
        &update(b"mode: rule\n", None),
    )
    .await
    .unwrap_err();
    assert!(matches!(stale, PortError::Rejected(_)));
    assert_eq!(
        read(store.config_dir().join("main.yaml")).await.unwrap(),
        before_stale_attempt
    );
}

#[tokio::test]
async fn absent_and_present_empty_identities_cross_reject_without_writes() {
    let dir = TempDir::new().unwrap();
    let store = manager(&dir);
    store
        .save_inactive_profile("main", "mode: rule\n")
        .await
        .unwrap();
    let absent = store
        .engine_profile_store()
        .load_workspace(&profile_id())
        .await
        .unwrap();
    let sidecar = options_path(store.config_dir(), "main");
    create_dir_all(sidecar.parent().unwrap()).await.unwrap();
    write(&sidecar, b"").await.unwrap();
    let present_empty = store
        .engine_profile_store()
        .load_workspace(&profile_id())
        .await
        .unwrap();
    let profile_path = store.config_dir().join("main.yaml");
    let profile_bytes = read(&profile_path).await.unwrap();

    let failure = Failure::from(
        store
            .engine_profile_store()
            .compare_and_save_inactive_workspace(
                &absent.identity(),
                &update(b"mode: global\n", Some(b"replacement")),
            )
            .await
            .unwrap_err(),
    );
    assert_eq!(failure.code, ErrorCode::NotReady);
    assert_eq!(read(&profile_path).await.unwrap(), profile_bytes);
    assert_eq!(read(&sidecar).await.unwrap(), b"");

    remove_file(&sidecar).await.unwrap();
    let failure = Failure::from(
        store
            .engine_profile_store()
            .compare_and_save_inactive_workspace(
                &present_empty.identity(),
                &update(b"mode: global\n", Some(b"replacement")),
            )
            .await
            .unwrap_err(),
    );
    assert_eq!(failure.code, ErrorCode::NotReady);
    assert_eq!(read(&profile_path).await.unwrap(), profile_bytes);
    assert!(!sidecar.exists());
}

#[tokio::test]
async fn protected_document_requires_explicit_direct_edit_unlock() {
    let dir = TempDir::new().unwrap();
    let store = manager(&dir);
    store
        .save_inactive_profile("main", "mode: rule\n")
        .await
        .unwrap();
    write(
        store.config_dir().join("profile-state.toml"),
        "[profiles.main]\nsubscription_url = \"https://one.example/sub\"\n",
    )
    .await
    .unwrap();
    let current = store
        .engine_profile_store()
        .load_workspace(&profile_id())
        .await
        .unwrap();
    let failure = Failure::from(
        store
            .engine_profile_store()
            .compare_and_save_inactive_workspace(
                &current.identity(),
                &direct_update(
                    b"mode: global\n",
                    None,
                    ProfileWriteAuthorization::RequireUnlock,
                ),
            )
            .await
            .unwrap_err(),
    );
    assert_eq!(failure.code, ErrorCode::Configuration);
    assert_eq!(store.load("main").await.unwrap(), "mode: rule\n");

    let unlocked = store
        .engine_profile_store()
        .compare_and_save_inactive_workspace(
            &current.identity(),
            &direct_update(
                b"mode: global\n",
                None,
                ProfileWriteAuthorization::ExplicitUnlock,
            ),
        )
        .await
        .unwrap();
    assert_eq!(unlocked.document.raw_bytes, b"mode: global\n");
}

#[tokio::test]
async fn changed_subscription_source_rejects_stale_workspace_before_writing() {
    let dir = TempDir::new().unwrap();
    let store = manager(&dir);
    store
        .save_inactive_profile("main", "mode: rule\n")
        .await
        .unwrap();
    let sidecar = options_path(store.config_dir(), "main");
    create_dir_all(sidecar.parent().unwrap()).await.unwrap();
    write(&sidecar, b"# retain current sidecar\r\n")
        .await
        .unwrap();
    let settings = store.config_dir().join("profile-state.toml");
    write(
        &settings,
        "[profiles.main]\nsubscription_url = \"https://one.example/sub\"\n",
    )
    .await
    .unwrap();
    let original = store
        .engine_profile_store()
        .load_workspace(&profile_id())
        .await
        .unwrap();
    assert_eq!(
        original.document.identity.source.kind,
        ProfileDocumentSourceKind::Subscription
    );
    write(
        &settings,
        "[profiles.main]\nsubscription_url = \"https://two.example/sub\"\n",
    )
    .await
    .unwrap();
    let profile_path = store.config_dir().join("main.yaml");
    let before = read(&profile_path).await.unwrap();

    let failure = Failure::from(
        store
            .engine_profile_store()
            .compare_and_save_inactive_workspace(
                &original.identity(),
                &update(b"mode: global\n", None),
            )
            .await
            .unwrap_err(),
    );

    assert_eq!(failure.code, ErrorCode::NotReady);
    assert_eq!(read(&profile_path).await.unwrap(), before);
    assert_eq!(
        read(&sidecar).await.unwrap(),
        b"# retain current sidecar\r\n"
    );
}

#[tokio::test]
async fn foreign_engine_workspace_is_rejected_before_profile_path_access() {
    let dir = TempDir::new().unwrap();
    let store = manager(&dir);
    let configs = dir.path().join("configs");
    write(&configs, "sentinel").await.unwrap();
    let profile = profile_id();
    let expected = foreign_identity(&profile);
    let previous = ProfileWorkspace {
        document: ProfileDocument::new(expected.document.clone(), b"mode: rule\n".to_vec()),
        sidecar_revision: ProfileSidecarRevision::Absent,
        write_protection: ProfileWriteProtection::Editable,
        sidecar_bytes: None,
    };

    let error = store
        .engine_profile_store()
        .compare_and_save_inactive_workspace(&expected, &update(b"mode: global\n", None))
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        PortError::Unsupported {
            capability: Capability::Profiles,
            ..
        }
    ));

    let error = store
        .engine_profile_store()
        .restore_inactive_workspace(&expected, &previous)
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        PortError::Unsupported {
            capability: Capability::Profiles,
            ..
        }
    ));
    assert_eq!(read(&configs).await.unwrap(), b"sentinel");
    assert!(!store.config_dir().join("profile-state.toml").exists());
}

#[tokio::test]
async fn unsupported_mihomo_format_and_schema_reject_before_profile_path_access() {
    let dir = TempDir::new().unwrap();
    let store = manager(&dir);
    let configs = store.config_dir().to_path_buf();
    create_dir_all(configs.parent().unwrap()).await.unwrap();
    write(&configs, b"sentinel").await.unwrap();

    for (format_id, schema_version) in [("sing-box-json", 1), (PROFILE_FORMAT, 2)] {
        let expected = mihomo_identity_with_format(&profile_id(), format_id, schema_version);
        let previous = ProfileWorkspace {
            document: ProfileDocument::new(expected.document.clone(), b"mode: rule\n".to_vec()),
            sidecar_revision: ProfileSidecarRevision::Absent,
            write_protection: ProfileWriteProtection::Editable,
            sidecar_bytes: None,
        };

        let compare_error = store
            .engine_profile_store()
            .compare_and_save_inactive_workspace(&expected, &update(b"mode: global\n", None))
            .await
            .unwrap_err();
        assert!(matches!(
            compare_error,
            PortError::Unsupported {
                capability: Capability::Profiles,
                ..
            }
        ));

        let restore_error = store
            .engine_profile_store()
            .restore_inactive_workspace(&expected, &previous)
            .await
            .unwrap_err();
        assert!(matches!(
            restore_error,
            PortError::Unsupported {
                capability: Capability::Profiles,
                ..
            }
        ));
        assert_eq!(read(&configs).await.unwrap(), b"sentinel");
        assert!(!store.config_dir().join("profile-state.toml").exists());
    }
}

#[tokio::test]
async fn document_and_sidecar_revision_must_match_the_exact_raw_bytes() {
    let dir = TempDir::new().unwrap();
    let store = manager(&dir);
    store
        .save_inactive_profile("main", "mode: rule\n")
        .await
        .unwrap();
    let current = store
        .engine_profile_store()
        .load_workspace(&profile_id())
        .await
        .unwrap();
    let profile_path = store.config_dir().join("main.yaml");
    let before = read(&profile_path).await.unwrap();
    let mut forged = current.clone();
    forged.document.raw_bytes = b"mode: direct\n".to_vec();

    let failure = Failure::from(
        store
            .engine_profile_store()
            .restore_inactive_workspace(&current.identity(), &forged)
            .await
            .unwrap_err(),
    );
    assert_eq!(failure.code, ErrorCode::InvalidInput);
    assert_eq!(read(&profile_path).await.unwrap(), before);

    let mut forged = current.clone();
    forged.sidecar_bytes = Some(b"# bytes without matching revision\n".to_vec());
    let failure = Failure::from(
        store
            .engine_profile_store()
            .restore_inactive_workspace(&current.identity(), &forged)
            .await
            .unwrap_err(),
    );
    assert_eq!(failure.code, ErrorCode::InvalidInput);
    assert_eq!(read(&profile_path).await.unwrap(), before);

    let sidecar = options_path(store.config_dir(), "main");
    create_dir_all(sidecar.parent().unwrap()).await.unwrap();
    write(&sidecar, b"\xff\x00#opaque\n").await.unwrap();
    let current = store
        .engine_profile_store()
        .load_workspace(&profile_id())
        .await
        .unwrap();
    assert_eq!(
        current.sidecar_bytes.as_deref(),
        Some(b"\xff\x00#opaque\n".as_slice())
    );
    assert_eq!(
        current.sidecar_revision,
        ProfileSidecarRevision::Present(document_revision(b"\xff\x00#opaque\n"))
    );
}

#[tokio::test]
async fn separate_managers_share_the_cas_boundary_and_only_one_proposal_commits() {
    let dir = TempDir::new().unwrap();
    let first = Arc::new(manager(&dir));
    first
        .save_inactive_profile("main", "mode: rule\n")
        .await
        .unwrap();
    let second = Arc::new(manager(&dir));
    let guard = first.lock_profile_writes().await.unwrap();
    assert!(
        timeout(
            Duration::from_millis(20),
            second.save_inactive_profile("main", "mode: direct\n")
        )
        .await
        .is_err()
    );
    drop(guard);
    assert_eq!(first.load("main").await.unwrap(), "mode: rule\n");

    let source = first
        .engine_profile_store()
        .load_workspace(&profile_id())
        .await
        .unwrap()
        .identity();
    let first_update = update(b"mode: global\n", Some(b"global-sidecar"));
    let second_update = update(b"mode: direct\n", Some(b"direct-sidecar"));
    let (left, right) = tokio::join!(
        first
            .engine_profile_store()
            .compare_and_save_inactive_workspace(&source, &first_update),
        second
            .engine_profile_store()
            .compare_and_save_inactive_workspace(&source, &second_update),
    );
    let (winner, rejected) = match (left, right) {
        (Ok(winner), Err(rejected)) | (Err(rejected), Ok(winner)) => (winner, rejected),
        results => panic!("exactly one same-source proposal may publish: {results:?}"),
    };
    assert_eq!(Failure::from(rejected).code, ErrorCode::NotReady);
    assert_eq!(
        first
            .engine_profile_store()
            .load_workspace(&profile_id())
            .await
            .unwrap(),
        winner
    );
    assert_eq!(
        second
            .engine_profile_store()
            .load_workspace(&profile_id())
            .await
            .unwrap(),
        winner
    );
    assert!(
        winner.sidecar_bytes.as_deref() == Some(b"global-sidecar".as_slice())
            || winner.sidecar_bytes.as_deref() == Some(b"direct-sidecar".as_slice())
    );
}

#[path = "profile_workspace_store_tests/part2.rs"]
mod part2;
