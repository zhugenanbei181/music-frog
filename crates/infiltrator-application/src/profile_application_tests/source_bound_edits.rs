//! Actual coherent store results, stale proposals and shared request state.
//! test-intent: behavior
use super::*;
use crate::profile_document_application::ProfileDocumentApplication;
use crate::profile_edit_session::ProfileEditSession;
use crate::profile_options_application::ProfileOptionsApplication;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::profile_document::ProfileDocumentSaved;

#[tokio::test]
async fn document_and_mixin_saves_refuse_the_users_old_source_before_touching_either_document() {
    let store = Arc::new(FakeStore::with_profile("main", "mode: rule\n", true));
    let profiles = ProfileApplication::new(store.clone());
    let document = ProfileDocumentApplication::new(profiles.clone());
    let mixin = ProfileOptionsApplication::new(profiles.clone());
    let observed = document.load(None).await.unwrap();
    store
        .save("main", "# external change\nmode: direct\n")
        .await
        .unwrap();
    let newer = profiles.load_workspace("main").await.unwrap();
    let source = observed.source.unwrap();
    let failure = document
        .save(
            None::<Arc<dyn ManagedRuntime>>,
            &source,
            "mode: global\n",
            false,
        )
        .await
        .unwrap_err();
    assert_eq!(failure.code, ErrorCode::NotReady);
    assert_eq!(profiles.load_workspace("main").await.unwrap(), newer);
    let failure = mixin
        .save_mixin(None::<Arc<dyn ManagedRuntime>>, &source, "mode: global\n")
        .await
        .unwrap_err();
    assert_eq!(failure.code, ErrorCode::NotReady);
    assert_eq!(profiles.load_workspace("main").await.unwrap(), newer);
    assert!(store.options.lock().unwrap().is_empty());
}

#[tokio::test]
async fn shared_editor_rejects_unit_and_old_terminal_keeps_draft_and_accepts_only_the_actual_retry_receipt()
 {
    let store = Arc::new(FakeStore::with_profile("main", "mode: rule\n", true));
    let profiles = ProfileApplication::new(store.clone());
    let document = ProfileDocumentApplication::new(profiles.clone());
    let observed = document.load(None).await.unwrap();
    let source = observed.source.clone().unwrap();
    let mut session = ProfileEditSession::default();
    assert!(session.observe(&source, &observed.content, ""));
    let old = session
        .begin_document("mode: global\n".into(), false)
        .unwrap();
    assert!(!session.can_edit());
    assert!(session.finish(old.operation, &old.intent, Ok(CommandOutput::Unit)));
    assert_eq!(
        session.failure.as_ref().unwrap().code,
        ErrorCode::InvalidState
    );
    assert_eq!(store.load("main").await.unwrap(), observed.content);
    assert!(session.dirty("mode: global\n"));
    let retry = session
        .begin_document("mode: global\n".into(), false)
        .unwrap();
    let committed = document
        .save(
            None::<Arc<dyn ManagedRuntime>>,
            &source,
            "mode: global\n",
            false,
        )
        .await
        .unwrap();
    let output = CommandOutput::ProfileDocumentSaved(Box::new(ProfileDocumentSaved {
        previous: source,
        document: committed.clone(),
    }));
    assert!(!session.finish(old.operation, &old.intent, Ok(output.clone())));
    assert!(session.pending.is_some());
    assert!(session.finish(retry.operation, &retry.intent, Ok(output)));
    assert_eq!(session.source(), committed.source.as_ref());
    assert!(session.saved);
    assert!(!session.dirty("mode: global\n"));
    let writes_before_discard = profiles.load_workspace("main").await.unwrap();
    assert_eq!(session.discard().as_deref(), Some("mode: global\n"));
    assert_eq!(
        profiles.load_workspace("main").await.unwrap(),
        writes_before_discard
    );
}

#[tokio::test]
async fn a_new_external_source_cannot_inherit_this_editors_prior_saved_claim() {
    let store = Arc::new(FakeStore::with_profile("main", "mode: rule\n", true));
    let profiles = ProfileApplication::new(store.clone());
    let document = ProfileDocumentApplication::new(profiles);
    let observed = document.load(None).await.unwrap();
    let source = observed.source.clone().unwrap();
    let mut session = ProfileEditSession::default();
    session.observe(&source, &observed.content, "");
    let pending = session
        .begin_document("mode: global\n".into(), false)
        .unwrap();
    let saved = document
        .save(
            None::<Arc<dyn ManagedRuntime>>,
            &source,
            "mode: global\n",
            false,
        )
        .await
        .unwrap();
    session.finish(
        pending.operation,
        &pending.intent,
        Ok(CommandOutput::ProfileDocumentSaved(Box::new(
            ProfileDocumentSaved {
                previous: source,
                document: saved.clone(),
            },
        ))),
    );
    assert!(session.saved);
    store
        .save("main", "# external editor\nmode: direct\n")
        .await
        .unwrap();
    let newer = document.load(None).await.unwrap();
    assert!(session.observe(
        newer.source.as_ref().unwrap(),
        &newer.content,
        &saved.content
    ));
    assert_eq!(session.source(), newer.source.as_ref());
    assert!(
        !session.saved,
        "an external write is a read observation, never this edit operation's success"
    );
    assert!(!session.dirty(&newer.content));
}
