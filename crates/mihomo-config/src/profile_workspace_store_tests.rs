//! test-intent: behavior
use super::*;
use crate::manager::ActiveProfileReferenceResetRequest;
use crate::profile_document_codec::{PROFILE_FORMAT, PROFILE_SCHEMA_VERSION};
use crate::sidecar_store::{write_document, write_document_bytes};
use async_trait::async_trait;
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReferenceCasOutcome, ActiveProfileReferenceState, ActiveProfileReferenceTarget,
    ActiveProfileReferenceWriteAuthorization,
};
use infiltrator_contract::core_identity::EngineId;
use infiltrator_contract::error::Failure;
use infiltrator_contract::profile_document_identity::{
    ProfileDocumentIdentity, ProfileDocumentSourceIdentity, ProfileDocumentSourceKind, ProfileId,
};
use infiltrator_contract::profile_protection::ProfileWriteAuthorization;
use infiltrator_domain::profile_options::options_path;
use infiltrator_ports::active_profile_reference_store::ActiveProfileReferenceStore;
use infiltrator_ports::profile_store::{
    ActiveProfileWorkspacePublishOutcome, ActiveProfileWorkspacePublishedLease,
    ActiveProfileWorkspaceRecoveryResolution, ActiveProfileWorkspaceRestoreOutcome,
    ActiveProfileWorkspaceWriteLease, ProfileStore,
};
use infiltrator_ports::secure_store::SecureStore;
use sha2::{Digest, Sha256};
use std::str;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::fs::{create_dir_all, read, remove_file, write};
use tokio::time::timeout;

struct TestStore;

#[async_trait]
impl SecureStore for TestStore {
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

fn manager(dir: &TempDir) -> ConfigManager<TestStore> {
    ConfigManager::with_home_and_store(dir.path().to_path_buf(), TestStore).unwrap()
}

async fn select_profile<S: SecureStore>(manager: &ConfigManager<S>, profile: &str) {
    let expected = ProfileStore::read_active_profile_reference(manager)
        .await
        .expect("read selection before test CAS");
    let target = ActiveProfileReferenceTarget::selected(
        ConfigManager::engine_id(manager),
        ProfileId::new(profile).expect("valid test profile ID"),
    );
    match ProfileStore::compare_and_set_active_profile_reference(
        manager,
        expected,
        target,
        infiltrator_contract::active_profile_reference::ActiveProfileReferenceWriteAuthorization::Unowned,
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
        revision(b"mode: rule\n"),
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
        revision(b"mode: rule\n"),
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
    let original = store.load_workspace(&profile_id()).await.unwrap();
    assert_eq!(original.sidecar_revision, ProfileSidecarRevision::Absent);
    assert_eq!(original.sidecar_bytes, None);
    assert_eq!(
        original.document.identity.revision,
        revision(b"mode: rule\n")
    );

    let sidecar = options_path(store.config_dir(), "main");
    create_dir_all(sidecar.parent().unwrap()).await.unwrap();
    write(&sidecar, b"").await.unwrap();
    let empty = store.load_workspace(&profile_id()).await.unwrap();
    assert_ne!(empty.identity(), original.identity());
    assert_eq!(empty.sidecar_bytes, Some(Vec::new()));
    assert_eq!(
        empty.sidecar_revision,
        ProfileSidecarRevision::Present(revision(b""))
    );

    let annotated = b"# exact annotation\r\n{}\r\n";
    write(&sidecar, annotated).await.unwrap();
    let with_comments = store.load_workspace(&profile_id()).await.unwrap();
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
    let edited = store.load_workspace(&profile_id()).await.unwrap();
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
    let original = store.load_workspace(&profile_id()).await.unwrap();
    store
        .save_inactive_profile("main", "# newer\nmode: direct\n")
        .await
        .unwrap();
    let changed = store.load_workspace(&profile_id()).await.unwrap();

    let failure = Failure::from(
        store
            .compare_and_save_inactive_workspace(
                &original.identity(),
                &update(b"mode: global\n", None),
            )
            .await
            .unwrap_err(),
    );
    assert_eq!(failure.code, ErrorCode::NotReady);
    assert_eq!(store.load_workspace(&profile_id()).await.unwrap(), changed);

    let sidecar = options_path(store.config_dir(), "main");
    create_dir_all(sidecar.parent().unwrap()).await.unwrap();
    write(&sidecar, b"filter: [old]\n").await.unwrap();
    let sidecar_changed = store.load_workspace(&profile_id()).await.unwrap();
    let profile_path = store.config_dir().join("main.yaml");
    let profile_before = read(&profile_path).await.unwrap();
    let sidecar_before = read(&sidecar).await.unwrap();
    let failure = Failure::from(
        store
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
        store.load_workspace(&profile_id()).await.unwrap(),
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
    let previous = store.load_workspace(&profile_id()).await.unwrap();

    let committed = store
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
        ProfileStore::read_active_profile_reference(&store)
            .await
            .unwrap()
    else {
        panic!("test profile must be selected");
    };
    let observed = store.load_active_workspace().await.unwrap();
    let sidecar_path = options_path(store.config_dir(), "main");
    create_dir_all(sidecar_path.parent().unwrap())
        .await
        .unwrap();
    write(&sidecar_path, b"filter: [keep]\n").await.unwrap();
    let sidecar_observed = store.load_active_workspace().await.unwrap();
    let committed = ProfileStore::compare_and_save_unowned_active_derived_document(
        &store,
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
        &store,
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
    let absent = store.load_workspace(&profile_id()).await.unwrap();
    let sidecar = options_path(store.config_dir(), "main");
    create_dir_all(sidecar.parent().unwrap()).await.unwrap();
    write(&sidecar, b"").await.unwrap();
    let present_empty = store.load_workspace(&profile_id()).await.unwrap();
    let profile_path = store.config_dir().join("main.yaml");
    let profile_bytes = read(&profile_path).await.unwrap();

    let failure = Failure::from(
        store
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
    let current = store.load_workspace(&profile_id()).await.unwrap();
    let failure = Failure::from(
        store
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
    let original = store.load_workspace(&profile_id()).await.unwrap();
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
    let store = ConfigManager::with_home_and_store(dir.path().to_path_buf(), TestStore).unwrap();
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
    let store = ConfigManager::with_home_and_store(dir.path().to_path_buf(), TestStore).unwrap();
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
    let current = store.load_workspace(&profile_id()).await.unwrap();
    let profile_path = store.config_dir().join("main.yaml");
    let before = read(&profile_path).await.unwrap();
    let mut forged = current.clone();
    forged.document.raw_bytes = b"mode: direct\n".to_vec();

    let failure = Failure::from(
        store
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
            .restore_inactive_workspace(&current.identity(), &forged)
            .await
            .unwrap_err(),
    );
    assert_eq!(failure.code, ErrorCode::InvalidInput);
    assert_eq!(read(&profile_path).await.unwrap(), before);

    let sidecar = options_path(store.config_dir(), "main");
    create_dir_all(sidecar.parent().unwrap()).await.unwrap();
    write(&sidecar, b"\xff\x00#opaque\n").await.unwrap();
    let current = store.load_workspace(&profile_id()).await.unwrap();
    assert_eq!(
        current.sidecar_bytes.as_deref(),
        Some(b"\xff\x00#opaque\n".as_slice())
    );
    assert_eq!(
        current.sidecar_revision,
        ProfileSidecarRevision::Present(revision(b"\xff\x00#opaque\n"))
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
        .load_workspace(&profile_id())
        .await
        .unwrap()
        .identity();
    let first_update = update(b"mode: global\n", Some(b"global-sidecar"));
    let second_update = update(b"mode: direct\n", Some(b"direct-sidecar"));
    let (left, right) = tokio::join!(
        first.compare_and_save_inactive_workspace(&source, &first_update),
        second.compare_and_save_inactive_workspace(&source, &second_update),
    );
    let (winner, rejected) = match (left, right) {
        (Ok(winner), Err(rejected)) | (Err(rejected), Ok(winner)) => (winner, rejected),
        results => panic!("exactly one same-source proposal may publish: {results:?}"),
    };
    assert_eq!(Failure::from(rejected).code, ErrorCode::NotReady);
    assert_eq!(first.load_workspace(&profile_id()).await.unwrap(), winner);
    assert_eq!(second.load_workspace(&profile_id()).await.unwrap(), winner);
    assert!(
        winner.sidecar_bytes.as_deref() == Some(b"global-sidecar".as_slice())
            || winner.sidecar_bytes.as_deref() == Some(b"direct-sidecar".as_slice())
    );
}

#[tokio::test]
async fn failed_document_publication_restores_exact_sidecar_bytes_or_absence() {
    let dir = TempDir::new().unwrap();
    let sidecar = dir.path().join("options/main.yaml");
    let previous_values: [Option<&[u8]>; 3] =
        [None, Some(b""), Some(b"# byte-exact comment\r\n\xff\x00")];
    for previous in previous_values {
        if let Some(previous) = previous {
            write_document_bytes(&sidecar, previous).await.unwrap();
        }
        let failure = publish_pair(&sidecar, previous, Some(b"new sidecar"), async {
            Err(PortError::PermissionDenied(
                "profile replacement denied".into(),
            ))
        })
        .await
        .unwrap_err();
        assert!(matches!(failure, PortError::PermissionDenied(_)));
        assert_eq!(
            read_optional_bytes(&sidecar).await.unwrap().as_deref(),
            previous
        );
    }
}

#[tokio::test]
async fn rollback_failure_retains_both_causes_without_claiming_clean_rejection() {
    let dir = TempDir::new().unwrap();
    let sidecar = dir.path().join("options/main.yaml");
    write_document_bytes(&sidecar, b"# old\n\xff")
        .await
        .unwrap();
    let failure = publish_pair(&sidecar, Some(b"# old\n\xff"), Some(b"new"), async {
        remove_file(&sidecar).await.unwrap();
        create_dir_all(&sidecar).await.unwrap();
        Err(PortError::Io("profile publication refused".into()))
    })
    .await
    .unwrap_err();
    let failure = Failure::from(failure);
    assert_eq!(failure.code, ErrorCode::InvalidState);
    assert!(!failure.retryable);
    assert!(failure.message.contains("profile publication refused"));
    assert!(failure.message.contains("sidecar rollback failed"));
    assert!(sidecar.is_dir());
}

#[tokio::test]
async fn active_and_inactive_publications_check_selection_inside_the_write_boundary() {
    let dir = TempDir::new().unwrap();
    let store = manager(&dir);
    store
        .save_inactive_profile("main", "mode: rule\n")
        .await
        .unwrap();
    store
        .save_inactive_profile("other", "mode: rule\n")
        .await
        .unwrap();
    select_profile(&store, "main").await;
    let other_id = ProfileId::new("other").unwrap();
    let other = store.load_workspace(&other_id).await.unwrap();
    let selected = match store.read_active_profile_reference().await.unwrap() {
        ActiveProfileReferenceState::Selected(reference) => reference,
        state => panic!("expected selected profile, got {state:?}"),
    };
    let error = match store
        .acquire_active_workspace_write_lease(
            selected,
            other.identity(),
            ActiveProfileReferenceWriteAuthorization::Unowned,
        )
        .await
    {
        Ok(_) => panic!("active workspace lease accepted an inactive workspace identity"),
        Err(error) => error,
    };
    let failure = Failure::from(error);
    assert_eq!(failure.code, ErrorCode::NotReady);
    assert_eq!(store.load_workspace(&other_id).await.unwrap(), other);

    select_profile(&store, "other").await;
    let failure = Failure::from(
        store
            .compare_and_save_inactive_workspace(
                &other.identity(),
                &update(b"mode: global\n", None),
            )
            .await
            .unwrap_err(),
    );
    assert_eq!(failure.code, ErrorCode::NotReady);
    let selected = match store.read_active_profile_reference().await.unwrap() {
        ActiveProfileReferenceState::Selected(reference) => reference,
        state => panic!("expected selected profile, got {state:?}"),
    };
    let active_lease = store
        .acquire_active_workspace_write_lease(
            selected,
            other.identity(),
            ActiveProfileReferenceWriteAuthorization::Unowned,
        )
        .await
        .unwrap();
    let committed = publish_or_panic(active_lease, update(b"mode: global\n", None))
        .await
        .commit();
    assert_eq!(store.load_active_workspace().await.unwrap(), committed);
    assert!(!store.config_dir().join("other.yaml.bak").exists());
    assert_eq!(store.load("main").await.unwrap(), "mode: rule\n");
}

#[tokio::test]
async fn managed_workspace_lease_holds_reference_through_restore_and_native_recovery() {
    let dir = TempDir::new().unwrap();
    let first = manager(&dir);
    let second = manager(&dir);
    first
        .save_inactive_profile("main", "mode: rule\n")
        .await
        .unwrap();
    first
        .save_inactive_profile("other", "mode: rule\n")
        .await
        .unwrap();
    select_profile(&first, "main").await;

    let reference = match first.read_active_profile_reference().await.unwrap() {
        ActiveProfileReferenceState::Selected(reference) => reference,
        state => panic!("expected selected profile, got {state:?}"),
    };
    let previous = first.load_workspace(&reference.profile_id).await.unwrap();
    let observed = first
        .acquire_active_workspace_write_lease(
            reference.clone(),
            previous.identity(),
            ActiveProfileReferenceWriteAuthorization::Unowned,
        )
        .await
        .unwrap();
    assert_eq!(observed.observed_workspace(), &previous);

    let blocked_cas = tokio::time::timeout(
        Duration::from_millis(30),
        second.compare_and_set_active_profile_reference(
            ActiveProfileReferenceState::Selected(reference.clone()),
            ActiveProfileReferenceTarget::selected(
                ConfigManager::engine_id(&second),
                ProfileId::new("other").unwrap(),
            ),
            ActiveProfileReferenceWriteAuthorization::Unowned,
        ),
    )
    .await;
    assert!(
        blocked_cas.is_err(),
        "selection must wait for the active transaction"
    );

    let published = publish_or_panic(observed, update(b"mode: global\n", None)).await;
    assert_eq!(
        tokio::fs::read(first.config_dir().join("main.yaml"))
            .await
            .unwrap(),
        b"mode: global\n"
    );
    let restored = match published.restore_if_current().await.unwrap() {
        ActiveProfileWorkspaceRestoreOutcome::Restored(restored) => restored,
        ActiveProfileWorkspaceRestoreOutcome::Superseded { observed } => {
            panic!("workspace unexpectedly superseded under its lease: {observed:?}")
        }
        ActiveProfileWorkspaceRestoreOutcome::RecoveryRequired(recovery) => {
            panic!(
                "workspace restore requires recovery: {}",
                recovery.last_error()
            )
        }
    };
    assert_eq!(restored.restored_workspace(), &previous);
    assert_eq!(
        tokio::fs::read(first.config_dir().join("main.yaml"))
            .await
            .unwrap(),
        b"mode: rule\n"
    );

    let blocked_until_native_restore = tokio::time::timeout(
        Duration::from_millis(30),
        second.compare_and_set_active_profile_reference(
            ActiveProfileReferenceState::Selected(reference.clone()),
            ActiveProfileReferenceTarget::selected(
                ConfigManager::engine_id(&second),
                ProfileId::new("other").unwrap(),
            ),
            ActiveProfileReferenceWriteAuthorization::Unowned,
        ),
    )
    .await;
    assert!(
        blocked_until_native_restore.is_err(),
        "selection remains fenced until the caller confirms native recovery"
    );
    restored.finish();

    assert!(matches!(
        second
            .compare_and_set_active_profile_reference(
                ActiveProfileReferenceState::Selected(reference),
                ActiveProfileReferenceTarget::selected(
                    ConfigManager::engine_id(&second),
                    ProfileId::new("other").unwrap(),
                ),
                ActiveProfileReferenceWriteAuthorization::Unowned,
            )
            .await
            .unwrap(),
        ActiveProfileReferenceCasOutcome::Committed { .. }
    ));
}

#[tokio::test]
async fn reset_commits_default_reference_before_cleaning_profile_files() {
    let dir = TempDir::new().unwrap();
    let store = manager(&dir);
    store
        .save_inactive_profile("default", "mode: rule\n")
        .await
        .unwrap();
    store
        .save_inactive_profile("other", "mode: global\n")
        .await
        .unwrap();
    let previous = store.read_active_profile_reference().await.unwrap();
    let default_content = "mode: direct\n";
    let expected_revision = previous.revision().next().unwrap();

    let outcome = store
        .reset_profiles_to_default(ActiveProfileReferenceResetRequest::Start {
            expected: previous.clone(),
            default_document: default_content.to_owned(),
        })
        .await
        .unwrap();
    let current = match outcome {
        crate::manager::ActiveProfileReferenceResetOutcome::Committed {
            previous: committed_previous,
            current,
        } => {
            assert_eq!(committed_previous, previous);
            current
        }
        other => panic!("reset did not complete: {other:?}"),
    };
    assert!(matches!(
        &current,
        ActiveProfileReferenceState::Selected(reference)
            if reference.engine_id == ConfigManager::engine_id(&store)
                && reference.profile_id.as_str() == "default"
                && reference.revision == expected_revision
    ));
    assert_eq!(
        store.read_active_profile_reference().await.unwrap(),
        current
    );
    assert_eq!(store.load("default").await.unwrap(), default_content);
    assert!(!store.config_dir().join("other.yaml").exists());

    assert!(matches!(
        store
            .reset_profiles_to_default(ActiveProfileReferenceResetRequest::Start {
                expected: previous.clone(),
                default_document: default_content.to_owned(),
            })
            .await
            .unwrap(),
        crate::manager::ActiveProfileReferenceResetOutcome::Committed {
            previous: retried_previous,
            current: retried_current,
        } if retried_previous == previous && retried_current == current
    ));
}

#[tokio::test]
async fn reset_of_an_already_default_profile_advances_revision_and_completes_intent() {
    let dir = TempDir::new().unwrap();
    let store = manager(&dir);
    let default_content = "mode: direct\n";
    store
        .save_inactive_profile("default", default_content)
        .await
        .unwrap();
    store
        .save_inactive_profile("other", "mode: global\n")
        .await
        .unwrap();
    select_profile(&store, "default").await;
    let expected = store.read_active_profile_reference().await.unwrap();
    let expected_next_revision = expected.revision().next().unwrap();

    let current = match store
        .reset_profiles_to_default(ActiveProfileReferenceResetRequest::Start {
            expected: expected.clone(),
            default_document: default_content.to_owned(),
        })
        .await
        .unwrap()
    {
        crate::manager::ActiveProfileReferenceResetOutcome::Committed { previous, current } => {
            assert_eq!(previous, expected);
            current
        }
        other => panic!("already-default reset did not complete: {other:?}"),
    };

    assert!(matches!(
        &current,
        ActiveProfileReferenceState::Selected(reference)
            if reference.engine_id == ConfigManager::engine_id(&store)
                && reference.profile_id.as_str() == "default"
                && reference.revision == expected_next_revision
    ));
    assert_eq!(
        store.read_active_profile_reference().await.unwrap(),
        current
    );
    assert_eq!(store.load("default").await.unwrap(), default_content);
    assert!(!store.config_dir().join("other.yaml").exists());
    assert!(
        !store
            .config_dir()
            .join(".active-profile-reset-intent.json")
            .exists()
    );
    assert!(
        store
            .config_dir()
            .join(".active-profile-reset-complete.json")
            .exists()
    );
}

#[tokio::test]
async fn reset_resumes_already_default_cleanup_from_its_durable_intent() {
    let dir = TempDir::new().unwrap();
    let store = manager(&dir);
    let default_content = "mode: direct\n";
    store
        .save_inactive_profile("default", default_content)
        .await
        .unwrap();
    store
        .save_inactive_profile("other", "mode: global\n")
        .await
        .unwrap();
    select_profile(&store, "default").await;
    let expected = store.read_active_profile_reference().await.unwrap();
    select_profile(&store, "default").await;
    let current = store.read_active_profile_reference().await.unwrap();
    assert_eq!(current.revision(), expected.revision().next().unwrap());

    let digest = Sha256::digest(default_content.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let intent = serde_json::json!({
        "schema_version": 1,
        "expected_reference": expected,
        "target_reference": current,
        "default_document_sha256": digest,
    });
    let intent_path = store.config_dir().join(".active-profile-reset-intent.json");
    write(&intent_path, serde_json::to_vec(&intent).unwrap())
        .await
        .unwrap();

    let outcome = store
        .reset_profiles_to_default(ActiveProfileReferenceResetRequest::Start {
            expected: expected.clone(),
            default_document: default_content.to_owned(),
        })
        .await
        .unwrap();
    assert!(matches!(
        outcome,
        crate::manager::ActiveProfileReferenceResetOutcome::Committed {
            previous,
            current: committed,
        } if previous == expected && committed == current
    ));
    assert!(!store.config_dir().join("other.yaml").exists());
    assert!(!intent_path.exists());
    assert!(
        store
            .config_dir()
            .join(".active-profile-reset-complete.json")
            .exists()
    );
}

#[tokio::test]
async fn stale_reset_leaves_profile_tree_untouched() {
    let dir = TempDir::new().unwrap();
    let store = manager(&dir);
    store
        .save_inactive_profile("default", "mode: rule\n")
        .await
        .unwrap();
    store
        .save_inactive_profile("other", "mode: global\n")
        .await
        .unwrap();
    let stale = store.read_active_profile_reference().await.unwrap();
    let default_before = tokio::fs::read(store.config_dir().join("default.yaml"))
        .await
        .unwrap();
    let other_before = tokio::fs::read(store.config_dir().join("other.yaml"))
        .await
        .unwrap();
    select_profile(&store, "other").await;

    assert!(matches!(
        store
            .reset_profiles_to_default(ActiveProfileReferenceResetRequest::Start {
                expected: stale.clone(),
                default_document: "mode: direct\n".to_owned(),
            })
            .await
            .unwrap(),
        crate::manager::ActiveProfileReferenceResetOutcome::Stale { expected, .. }
            if expected == stale
    ));
    assert_eq!(
        tokio::fs::read(store.config_dir().join("default.yaml"))
            .await
            .unwrap(),
        default_before
    );
    assert_eq!(
        tokio::fs::read(store.config_dir().join("other.yaml"))
            .await
            .unwrap(),
        other_before
    );
}

#[tokio::test]
async fn reset_rejects_a_bound_owner_and_preserves_profile_files() {
    let dir = TempDir::new().unwrap();
    let store = manager(&dir);
    store
        .save_inactive_profile("default", "mode: rule\n")
        .await
        .unwrap();
    store
        .save_inactive_profile("other", "mode: global\n")
        .await
        .unwrap();
    select_profile(&store, "default").await;
    let expected = store.read_active_profile_reference().await.unwrap();
    let previous_default = tokio::fs::read(store.config_dir().join("default.yaml"))
        .await
        .unwrap();
    let previous_other = tokio::fs::read(store.config_dir().join("other.yaml"))
        .await
        .unwrap();

    let selection = ActiveProfileReferenceStore::acquire_selection_lease(&store)
        .await
        .unwrap();
    let identity = infiltrator_contract::core_identity::CoreIdentity::new(
        ConfigManager::engine_id(&store),
        infiltrator_contract::core_identity::CoreInstanceId::new("reset-owner").unwrap(),
        None,
    );
    let owner = selection
        .into_owner_binding(
            identity.clone(),
            infiltrator_contract::core_identity::CoreGeneration::new(0),
        )
        .await
        .unwrap();

    let failure = Failure::from(
        store
            .reset_profiles_to_default(ActiveProfileReferenceResetRequest::Start {
                expected,
                default_document: "mode: direct\n".to_owned(),
            })
            .await
            .unwrap_err(),
    );
    assert_eq!(failure.code, ErrorCode::NotReady);
    assert_eq!(
        tokio::fs::read(store.config_dir().join("default.yaml"))
            .await
            .unwrap(),
        previous_default
    );
    assert_eq!(
        tokio::fs::read(store.config_dir().join("other.yaml"))
            .await
            .unwrap(),
        previous_other
    );

    owner
        .release_after_terminal(&infiltrator_contract::snapshot::CoreLifecycleSnapshot {
            identity: Some(identity),
            lifecycle: infiltrator_contract::snapshot::CoreLifecycle::Stopped,
            generation: 0,
            session_token: None,
            revision: 0,
            failure: None,
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn exact_recovery_rejects_forged_or_later_documents_and_restores_raw_bytes() {
    let dir = TempDir::new().unwrap();
    let store = manager(&dir);
    let previous_document = b"# preserve document bytes\r\nmode: rule\r\n";
    store
        .save_inactive_profile("main", str::from_utf8(previous_document).unwrap())
        .await
        .unwrap();
    let path = options_path(store.config_dir(), "main");
    create_dir_all(path.parent().unwrap()).await.unwrap();
    let previous_sidecar = b"# retain exact annotation\r\n\xff\x00";
    write(&path, previous_sidecar).await.unwrap();
    let previous = store.load_workspace(&profile_id()).await.unwrap();
    let committed = store
        .compare_and_save_inactive_workspace(
            &previous.identity(),
            &update(b"mode: global\n", Some(b"new-sidecar")),
        )
        .await
        .unwrap();
    let mut forged = previous.clone();
    forged.document.raw_bytes = b"mode: direct\n".to_vec();
    assert_eq!(
        Failure::from(
            store
                .restore_inactive_workspace(&committed.identity(), &forged)
                .await
                .unwrap_err()
        )
        .code,
        ErrorCode::InvalidInput
    );
    assert_eq!(
        store.load_workspace(&profile_id()).await.unwrap(),
        committed
    );

    let recovered = store
        .restore_inactive_workspace(&committed.identity(), &previous)
        .await
        .unwrap();
    assert_eq!(recovered.document.raw_bytes, previous_document);
    assert_eq!(
        recovered.sidecar_bytes.as_deref(),
        Some(previous_sidecar.as_slice())
    );
    assert_eq!(read(&path).await.unwrap(), previous_sidecar);

    let later = store
        .compare_and_save_inactive_workspace(
            &recovered.identity(),
            &update(b"mode: direct\n", Some(previous_sidecar)),
        )
        .await
        .unwrap();
    assert_eq!(
        Failure::from(
            store
                .restore_inactive_workspace(&committed.identity(), &previous)
                .await
                .unwrap_err()
        )
        .code,
        ErrorCode::NotReady
    );
    assert_eq!(store.load_workspace(&profile_id()).await.unwrap(), later);
}

#[tokio::test]
async fn deleting_profile_removes_its_sidecar() {
    let dir = TempDir::new().unwrap();
    let store = manager(&dir);
    store
        .save_inactive_profile("main", "mode: rule\n")
        .await
        .unwrap();
    store
        .save_inactive_profile("other", "mode: rule\n")
        .await
        .unwrap();
    let sidecar = options_path(store.config_dir(), "other");
    create_dir_all(sidecar.parent().unwrap()).await.unwrap();
    write(&sidecar, b"# keep no orphan\n").await.unwrap();

    store.delete_profile("other").await.unwrap();

    assert!(!store.config_dir().join("other.yaml").exists());
    assert!(!sidecar.exists());
}
