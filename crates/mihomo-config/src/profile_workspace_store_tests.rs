//! test-intent: behavior
use super::*;
use async_trait::async_trait;
use infiltrator_domain::profile_options::{FilterSpec, ProfileOptions};
use infiltrator_ports::profile_store::ProfileStore;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::fs::{create_dir_all, read_to_string, remove_file, write};
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
fn proposed(value: &str) -> ProfileWorkspaceUpdate {
    ProfileWorkspaceUpdate {
        purpose: ProfileWorkspacePurpose::Derived,
        content: format!("mode: {value}\n"),
        options: ProfileOptions {
            filter: Some(FilterSpec {
                include_keywords: vec![value.into()],
                ..Default::default()
            }),
            ..Default::default()
        },
    }
}

#[tokio::test]
async fn workspace_identity_tracks_exact_documents_absence_and_empty_sidecar() {
    let dir = TempDir::new().unwrap();
    let store = manager(&dir);
    store.save("main", "mode: rule\n").await.unwrap();
    let original = store.load_workspace("main").await.unwrap();
    assert!(original.source.options_hash.is_none());
    let path = options_path(store.config_dir(), "main");
    create_dir_all(path.parent().unwrap()).await.unwrap();
    write(&path, "{}\n").await.unwrap();
    let empty = store.load_workspace("main").await.unwrap();
    assert_eq!(empty.options, original.options);
    assert_ne!(empty.source, original.source);
    write(&path, "# annotation\n{}\n").await.unwrap();
    let annotated = store.load_workspace("main").await.unwrap();
    assert_eq!(annotated.options, empty.options);
    assert_ne!(annotated.source.options_hash, empty.source.options_hash);
    store
        .save("main", "# user edit\nmode: rule\n")
        .await
        .unwrap();
    let edited = store.load_workspace("main").await.unwrap();
    assert_ne!(edited.source.document_hash, annotated.source.document_hash);
    assert_eq!(edited.source.options_hash, annotated.source.options_hash);
}

#[tokio::test]
async fn stale_document_or_sidecar_rejects_commit_before_either_file_changes() {
    let dir = TempDir::new().unwrap();
    let store = manager(&dir);
    store.save("main", "mode: rule\n").await.unwrap();
    let original = store.load_workspace("main").await.unwrap();
    store.save("main", "# newer\nmode: direct\n").await.unwrap();
    let changed = store.load_workspace("main").await.unwrap();
    let failure = Failure::from(
        store
            .compare_and_save_workspace(&original.source, &proposed("global"))
            .await
            .unwrap_err(),
    );
    assert_eq!(failure.code, ErrorCode::NotReady);
    assert_eq!(store.load_workspace("main").await.unwrap(), changed);
    store
        .save_options("main", &proposed("rule").options)
        .await
        .unwrap();
    let sidecar_changed = store.load_workspace("main").await.unwrap();
    let failure = Failure::from(
        store
            .compare_and_save_workspace(&changed.source, &proposed("global"))
            .await
            .unwrap_err(),
    );
    assert_eq!(failure.code, ErrorCode::NotReady);
    assert_eq!(store.load_workspace("main").await.unwrap(), sidecar_changed);
    let mut invalid = proposed("global");
    invalid.content = "mode: [unterminated".into();
    assert_eq!(
        Failure::from(
            store
                .compare_and_save_workspace(&sidecar_changed.source, &invalid)
                .await
                .unwrap_err()
        )
        .code,
        ErrorCode::Configuration
    );
    assert_eq!(store.load_workspace("main").await.unwrap(), sidecar_changed);
}

#[tokio::test]
async fn separate_managers_share_boundary_and_only_one_same_source_proposal_commits() {
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
    assert_eq!(first.load("main").await.unwrap(), "mode: rule\n");
    drop(guard);
    let source = first.load_workspace("main").await.unwrap().source;
    let global = proposed("global");
    let direct = proposed("direct");
    let (a, b) = tokio::join!(
        first.compare_and_save_workspace(&source, &global),
        second.compare_and_save_workspace(&source, &direct),
    );
    let (winner, rejected) = match (a, b) {
        (Ok(winner), Err(rejected)) | (Err(rejected), Ok(winner)) => (winner, rejected),
        results => panic!("exactly one proposal may publish: {results:?}"),
    };
    assert_eq!(Failure::from(rejected).code, ErrorCode::NotReady);
    assert_eq!(first.load_workspace("main").await.unwrap(), winner);
    assert_eq!(second.load_workspace("main").await.unwrap(), winner);
    assert!(
        winner
            .content
            .contains(&winner.options.filter.unwrap().include_keywords[0])
    );
}

#[tokio::test]
async fn failed_profile_publication_restores_exact_option_bytes_or_absence() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("main.yaml");
    for previous in [None, Some("# user annotation\n{}\n")] {
        if let Some(previous) = previous {
            write(&path, previous).await.unwrap();
        }
        let expected = PortError::PermissionDenied("profile replacement denied".into());
        let failure = publish_pair(&path, previous, Some("filter: {}\n"), async {
            Err(expected.clone())
        })
        .await
        .unwrap_err();
        assert_eq!(failure, expected);
        match previous {
            Some(previous) => assert_eq!(read_to_string(&path).await.unwrap(), previous),
            None => assert!(!path.exists()),
        }
    }
}

#[tokio::test]
async fn rollback_failure_retains_both_causes_and_never_claims_a_clean_rejection() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("main.yaml");
    write(&path, "# old\n{}\n").await.unwrap();
    let failure = publish_pair(&path, Some("# old\n{}\n"), Some("filter: {}\n"), async {
        remove_file(&path).await.unwrap();
        create_dir_all(&path).await.unwrap();
        Err(PortError::Io("profile publication refused".into()))
    })
    .await
    .unwrap_err();
    let failure = Failure::from(failure);
    assert_eq!(failure.code, ErrorCode::InvalidState);
    assert!(!failure.retryable);
    assert!(failure.message.contains("profile publication refused"));
    assert!(failure.message.contains("options rollback failed"));
    assert!(path.is_dir());
}

#[tokio::test]
async fn active_and_inactive_publications_check_selection_inside_the_write_boundary() {
    let dir = TempDir::new().unwrap();
    let store = manager(&dir);
    store.save("main", "mode: rule\n").await.unwrap();
    store.save("other", "mode: rule\n").await.unwrap();
    store
        .save("other", "# annotation\nmode: rule\n")
        .await
        .unwrap();
    assert!(store.load_backup("other").await.unwrap().is_some());
    store.set_current("main").await.unwrap();
    let other = store.load_workspace("other").await.unwrap();
    let failure = Failure::from(
        store
            .compare_and_save_active_workspace(&other.source, &proposed("global"))
            .await
            .unwrap_err(),
    );
    assert_eq!(failure.code, ErrorCode::NotReady);
    assert_eq!(store.load_workspace("other").await.unwrap(), other);
    store.set_current("other").await.unwrap();
    let failure = Failure::from(
        store
            .compare_and_save_inactive_workspace(&other.source, &proposed("global"))
            .await
            .unwrap_err(),
    );
    assert_eq!(failure.code, ErrorCode::NotReady);
    assert_eq!(store.load_active_workspace().await.unwrap(), other);
    let committed = store
        .compare_and_save_active_workspace(&other.source, &proposed("global"))
        .await
        .unwrap();
    assert_eq!(store.load_active_workspace().await.unwrap(), committed);
    assert!(store.load_backup("other").await.unwrap().is_none());
    assert_eq!(store.load("main").await.unwrap(), "mode: rule\n");
}

#[tokio::test]
async fn exact_recovery_validates_prior_identity_and_refuses_forged_or_later_documents() {
    let dir = TempDir::new().unwrap();
    let store = manager(&dir);
    store.save("main", "mode: rule\n").await.unwrap();
    let path = options_path(store.config_dir(), "main");
    create_dir_all(path.parent().unwrap()).await.unwrap();
    write(&path, "# retain exact annotation\n{}\n")
        .await
        .unwrap();
    let previous = store.load_workspace("main").await.unwrap();
    let committed = store
        .compare_and_save_workspace(&previous.source, &proposed("global"))
        .await
        .unwrap();
    let mut forged = previous.clone();
    forged.options.filter = proposed("direct").options.filter;
    assert_eq!(
        Failure::from(
            store
                .restore_workspace(&committed.source, &forged)
                .await
                .unwrap_err()
        )
        .code,
        ErrorCode::InvalidInput
    );
    assert_eq!(store.load_workspace("main").await.unwrap(), committed);
    let recovered = store
        .restore_workspace(&committed.source, &previous)
        .await
        .unwrap();
    assert_eq!(recovered, previous);
    assert_eq!(
        read_to_string(&path).await.unwrap(),
        "# retain exact annotation\n{}\n"
    );
    let latest = store
        .compare_and_save_workspace(&previous.source, &proposed("direct"))
        .await
        .unwrap();
    assert_eq!(
        Failure::from(
            store
                .restore_workspace(&committed.source, &previous)
                .await
                .unwrap_err()
        )
        .code,
        ErrorCode::NotReady
    );
    assert_eq!(store.load_workspace("main").await.unwrap(), latest);
}

#[tokio::test]
async fn content_only_workspace_commit_preserves_exact_unchanged_options_bytes() {
    let temp = TempDir::new().unwrap();
    let store = manager(&temp);
    store
        .save("main", "mode: rule\nproxies: []\n")
        .await
        .unwrap();
    let path = options_path(store.config_dir(), "main");
    create_dir_all(path.parent().unwrap()).await.unwrap();
    let raw = "# exact annotations and empty-document identity\r\n{}\r\n";
    write(&path, raw).await.unwrap();
    let before = store.load_workspace("main").await.unwrap();
    let update = ProfileWorkspaceUpdate {
        purpose: ProfileWorkspacePurpose::Derived,
        content: "mode: global\nproxies: []\n".into(),
        options: before.options.clone(),
    };
    let committed = store
        .compare_and_save_workspace(&before.source, &update)
        .await
        .unwrap();
    assert_eq!(read_to_string(&path).await.unwrap(), raw);
    assert_eq!(committed.source.options_hash, before.source.options_hash);
    assert_eq!(committed.options_document.as_deref(), Some(raw));
}

#[tokio::test]
async fn direct_edit_rechecks_new_provider_ownership_before_any_document_changes_and_unlock_is_explicit()
 {
    let dir = TempDir::new().unwrap();
    let store = manager(&dir);
    store.save("main", "mode: rule\n").await.unwrap();
    let before = store.load_workspace("main").await.unwrap();
    let mut metadata = store.get_profile_metadata("main").await.unwrap();
    metadata.subscription_url = Some("https://provider.test/subscription".into());
    store
        .update_profile_metadata("main", &metadata)
        .await
        .unwrap();
    let protected = store.load_workspace("main").await.unwrap();
    assert_eq!(
        protected.source, before.source,
        "ownership can change without changing either document"
    );
    assert!(
        protected.write_protection.is_protected(),
        "an unavailable secure-store value cannot erase persisted provider ownership"
    );
    let mut update = proposed("global");
    update.purpose = ProfileWorkspacePurpose::DirectEdit {
        allow_protected: false,
    };
    let failure = Failure::from(
        store
            .compare_and_save_workspace(&before.source, &update)
            .await
            .unwrap_err(),
    );
    assert_eq!(failure.code, ErrorCode::Configuration);
    assert_eq!(
        store.load_workspace("main").await.unwrap(),
        protected,
        "refused direct edit writes neither file"
    );
    update.purpose = ProfileWorkspacePurpose::DirectEdit {
        allow_protected: true,
    };
    let committed = store
        .compare_and_save_workspace(&before.source, &update)
        .await
        .unwrap();
    assert!(committed.write_protection.is_protected());
    assert_eq!(committed.content, update.content);
    assert_eq!(committed.options, update.options);
    assert_eq!(store.load_workspace("main").await.unwrap(), committed);
}
