//! test-intent: behavior
use super::*;
use async_trait::async_trait;
use infiltrator_contract::core_identity::EngineId;
use infiltrator_contract::error::Failure;
use infiltrator_contract::profile_document_identity::{
    ProfileDocumentIdentity, ProfileDocumentSourceIdentity, ProfileDocumentSourceKind,
};
use infiltrator_contract::profile_protection::ProfileWriteAuthorization;
use infiltrator_domain::profile_options::options_path;
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::secure_store::SecureStore;
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
    store.save("main", "mode: rule\n").await.unwrap();
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
        .save("main", str::from_utf8(edited_document).unwrap())
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
async fn stale_document_or_sidecar_rejects_before_either_file_changes() {
    let dir = TempDir::new().unwrap();
    let store = manager(&dir);
    store.save("main", "mode: rule\n").await.unwrap();
    let original = store.load_workspace(&profile_id()).await.unwrap();
    store.save("main", "# newer\nmode: direct\n").await.unwrap();
    let changed = store.load_workspace(&profile_id()).await.unwrap();

    let failure = Failure::from(
        store
            .compare_and_save_workspace(&original.identity(), &update(b"mode: global\n", None))
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
            .compare_and_save_workspace(
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
async fn absent_and_present_empty_identities_cross_reject_without_writes() {
    let dir = TempDir::new().unwrap();
    let store = manager(&dir);
    store.save("main", "mode: rule\n").await.unwrap();
    let absent = store.load_workspace(&profile_id()).await.unwrap();
    let sidecar = options_path(store.config_dir(), "main");
    create_dir_all(sidecar.parent().unwrap()).await.unwrap();
    write(&sidecar, b"").await.unwrap();
    let present_empty = store.load_workspace(&profile_id()).await.unwrap();
    let profile_path = store.config_dir().join("main.yaml");
    let profile_bytes = read(&profile_path).await.unwrap();

    let failure = Failure::from(
        store
            .compare_and_save_workspace(
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
            .compare_and_save_workspace(
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
    store.save("main", "mode: rule\n").await.unwrap();
    write(
        store.config_dir().join("profile-state.toml"),
        "[profiles.main]\nsubscription_url = \"https://one.example/sub\"\n",
    )
    .await
    .unwrap();
    let current = store.load_workspace(&profile_id()).await.unwrap();
    let failure = Failure::from(
        store
            .compare_and_save_workspace(
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
        .compare_and_save_workspace(
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
    store.save("main", "mode: rule\n").await.unwrap();
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
            .compare_and_save_workspace(&original.identity(), &update(b"mode: global\n", None))
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
        .compare_and_save_workspace(&expected, &update(b"mode: global\n", None))
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
        .restore_workspace(&expected, &previous)
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
            .compare_and_save_workspace(&expected, &update(b"mode: global\n", None))
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
            .restore_workspace(&expected, &previous)
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
    store.save("main", "mode: rule\n").await.unwrap();
    let current = store.load_workspace(&profile_id()).await.unwrap();
    let profile_path = store.config_dir().join("main.yaml");
    let before = read(&profile_path).await.unwrap();
    let mut forged = current.clone();
    forged.document.raw_bytes = b"mode: direct\n".to_vec();

    let failure = Failure::from(
        store
            .restore_workspace(&current.identity(), &forged)
            .await
            .unwrap_err(),
    );
    assert_eq!(failure.code, ErrorCode::InvalidInput);
    assert_eq!(read(&profile_path).await.unwrap(), before);

    let mut forged = current.clone();
    forged.sidecar_bytes = Some(b"# bytes without matching revision\n".to_vec());
    let failure = Failure::from(
        store
            .restore_workspace(&current.identity(), &forged)
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
    first.save("main", "mode: rule\n").await.unwrap();
    let second = Arc::new(manager(&dir));
    let guard = first.lock_profile_writes().await;
    assert!(
        timeout(
            Duration::from_millis(20),
            second.save("main", "mode: direct\n")
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
        first.compare_and_save_workspace(&source, &first_update),
        second.compare_and_save_workspace(&source, &second_update),
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
    store.save("main", "mode: rule\n").await.unwrap();
    store.save("other", "mode: rule\n").await.unwrap();
    store.set_current("main").await.unwrap();
    let other_id = ProfileId::new("other").unwrap();
    let other = store.load_workspace(&other_id).await.unwrap();
    let failure = Failure::from(
        store
            .compare_and_save_active_workspace(&other.identity(), &update(b"mode: global\n", None))
            .await
            .unwrap_err(),
    );
    assert_eq!(failure.code, ErrorCode::NotReady);
    assert_eq!(store.load_workspace(&other_id).await.unwrap(), other);

    store.set_current("other").await.unwrap();
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
    let committed = store
        .compare_and_save_active_workspace(&other.identity(), &update(b"mode: global\n", None))
        .await
        .unwrap();
    assert_eq!(store.load_active_workspace().await.unwrap(), committed);
    assert!(!store.config_dir().join("other.yaml.bak").exists());
    assert_eq!(store.load("main").await.unwrap(), "mode: rule\n");
}

#[tokio::test]
async fn exact_recovery_rejects_forged_or_later_documents_and_restores_raw_bytes() {
    let dir = TempDir::new().unwrap();
    let store = manager(&dir);
    let previous_document = b"# preserve document bytes\r\nmode: rule\r\n";
    store
        .save("main", str::from_utf8(previous_document).unwrap())
        .await
        .unwrap();
    let path = options_path(store.config_dir(), "main");
    create_dir_all(path.parent().unwrap()).await.unwrap();
    let previous_sidecar = b"# retain exact annotation\r\n\xff\x00";
    write(&path, previous_sidecar).await.unwrap();
    let previous = store.load_workspace(&profile_id()).await.unwrap();
    let committed = store
        .compare_and_save_workspace(
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
                .restore_workspace(&committed.identity(), &forged)
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
        .restore_workspace(&committed.identity(), &previous)
        .await
        .unwrap();
    assert_eq!(recovered.document.raw_bytes, previous_document);
    assert_eq!(
        recovered.sidecar_bytes.as_deref(),
        Some(previous_sidecar.as_slice())
    );
    assert_eq!(read(&path).await.unwrap(), previous_sidecar);

    let later = store
        .compare_and_save_workspace(
            &recovered.identity(),
            &update(b"mode: direct\n", Some(previous_sidecar)),
        )
        .await
        .unwrap();
    assert_eq!(
        Failure::from(
            store
                .restore_workspace(&committed.identity(), &previous)
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
    store.save("main", "mode: rule\n").await.unwrap();
    store.save("other", "mode: rule\n").await.unwrap();
    let sidecar = options_path(store.config_dir(), "other");
    create_dir_all(sidecar.parent().unwrap()).await.unwrap();
    write(&sidecar, b"# keep no orphan\n").await.unwrap();

    store.delete_profile("other").await.unwrap();

    assert!(!store.config_dir().join("other.yaml").exists());
    assert!(!sidecar.exists());
}
