//! Actual failed reads retain facts, independent source verification fences saves.
//! test-intent: behavior
use super::*;
use crate::profile_document_application::ProfileDocumentApplication;
use crate::profile_edit_session::ProfileEditSession;
use crate::profile_editor_observations::EditorFacet;
use crate::profile_options_application::ProfileOptionsApplication;
use infiltrator_contract::profile_editor_read::ProfileReadStatus;

#[tokio::test]
async fn failed_document_read_retains_both_facts_and_a_successful_options_read_cannot_erase_its_failure()
 {
    let store = Arc::new(FakeStore::with_profile("main", "mode: rule\n", true));
    let profiles = ProfileApplication::new(store.clone());
    let documents = ProfileDocumentApplication::new(profiles.clone());
    let options = ProfileOptionsApplication::new(profiles.clone());
    let document = documents.load(None).await.unwrap();
    let option = options.load(None).await.unwrap();
    let failure = PortError::PermissionDenied("denied document read".into());
    *store.options_read_error.lock().unwrap() = Some(failure.clone());
    assert_eq!(
        documents.load(None).await.unwrap_err().code,
        ErrorCode::Permission
    );
    let denied = profiles.verified_editor_snapshot().await;
    assert_eq!(denied.document, Some(document.clone()));
    assert_eq!(denied.options, Some(option));
    assert_eq!(
        denied.read.document,
        ProfileReadStatus::Failed(Failure::from(failure))
    );
    assert_eq!(denied.read.options, ProfileReadStatus::Ready);
    assert!(!denied.read.source_current());
    assert_eq!(
        denied.read.verification_failure.unwrap().code,
        ErrorCode::Permission
    );
    *store.options_read_error.lock().unwrap() = None;
    options.load(None).await.unwrap();
    let recovered_options = profiles.verified_editor_snapshot().await;
    assert!(recovered_options.read.source_current());
    assert!(matches!(
        recovered_options.read.document,
        ProfileReadStatus::Failed(_)
    ));
    let mut session = ProfileEditSession::default();
    session.observe(document.source.as_ref().unwrap(), &document.content, "");
    session.observe_read_status(&recovered_options.read, false);
    assert!(!session.can_save());
    assert!(
        session
            .status(&document.content, "en-US")
            .starts_with("Editor read failed:")
    );
    documents.load(None).await.unwrap();
    session.observe_read_status(&profiles.verified_editor_snapshot().await.read, false);
    assert!(session.can_save());
}

#[tokio::test]
async fn an_external_write_without_a_reload_is_verified_stale_and_does_not_replace_the_open_draft()
{
    let store = Arc::new(FakeStore::with_profile("main", "mode: rule\n", true));
    let profiles = ProfileApplication::new(store.clone());
    let document = ProfileDocumentApplication::new(profiles.clone())
        .load(None)
        .await
        .unwrap();
    let mut session = ProfileEditSession::default();
    session.observe(document.source.as_ref().unwrap(), &document.content, "");
    store
        .save("main", "# external\nmode: direct\n")
        .await
        .unwrap();
    let snapshot = profiles.verified_editor_snapshot().await;
    assert_eq!(snapshot.document, Some(document));
    assert_ne!(snapshot.read.observed_source, snapshot.read.verified_source);
    assert!(!snapshot.read.source_current());
    session.observe_read_status(&snapshot.read, false);
    assert!(!session.can_save());
    assert!(session.dirty("mode: global\n"));
    assert!(
        session
            .status("mode: global\n", "zh-CN")
            .contains("配置或选项已变化")
    );
}

#[tokio::test]
async fn late_read_failure_cannot_replace_a_newer_success_and_metadata_protection_is_verified_without_changing_bytes()
 {
    let store = Arc::new(FakeStore::with_profile("main", "mode: rule\n", true));
    let profiles = ProfileApplication::new(store.clone());
    let documents = ProfileDocumentApplication::new(profiles.clone());
    let document = documents.load(None).await.unwrap();
    let old = profiles.begin_editor_read("main", EditorFacet::Document);
    documents.load(None).await.unwrap();
    profiles.mark_editor_loading(&old);
    assert_eq!(
        profiles.editor_read_status().document,
        ProfileReadStatus::Ready
    );
    assert!(
        !profiles.fail_editor_read(&old, Failure::new(ErrorCode::Storage, "old failure", true))
    );
    mark_subscription(&store, "main", "https://provider.test/subscription");
    let snapshot = profiles.verified_editor_snapshot().await;
    assert_eq!(snapshot.read.document, ProfileReadStatus::Ready);
    assert!(snapshot.read.source_current());
    assert_eq!(
        snapshot.document.as_ref().unwrap().content,
        document.content
    );
    assert!(snapshot.document.unwrap().write_protection.is_protected());
}
