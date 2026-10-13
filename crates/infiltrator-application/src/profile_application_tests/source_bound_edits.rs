//! Actual coherent store results, stale proposals and shared request state.
//! test-intent: behavior
use super::*;
use crate::profile_application::ProfileRuntimeAccess;
use crate::profile_document_application::ProfileDocumentApplication;
use crate::profile_edit_session::{
    ProfileEditFinish, ProfileEditObservation, ProfileEditOperationPhase, ProfileEditSession,
};
use crate::profile_options_application::ProfileOptionsApplication;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::profile_document::ProfileDocumentSaved;
use infiltrator_contract::profile_editor_read::ProfileEditStatusFact;
use infiltrator_contract::profile_protection::ProfileWriteAuthorization;
use infiltrator_ports::active_profile_reference_applier::ActiveProfileReferenceApplier;
use std::sync::Arc;

#[tokio::test]
async fn document_and_mixin_saves_refuse_the_users_old_source_before_touching_either_document() {
    let store = Arc::new(FakeStore::with_profile("main", "mode: rule\n", true));
    let profiles = profiles_with_mihomo_codec(store.clone());
    let document = ProfileDocumentApplication::new(profiles.clone());
    let mixin = ProfileOptionsApplication::new(profiles.clone());
    let observed = document.load(None).await.unwrap();
    store.replace_document_for_test("main", "# external change\nmode: direct\n");
    let newer = profiles.load_workspace(&profile_id("main")).await.unwrap();
    let source = observed.workspace_identity;
    let failure = document
        .save(
            ProfileRuntimeAccess::<dyn ActiveProfileReferenceApplier>::Offline,
            &source,
            "mode: global\n",
            ProfileWriteAuthorization::RequireUnlock,
        )
        .await
        .unwrap_err();
    assert_eq!(failure.code, ErrorCode::NotReady);
    assert_eq!(
        profiles.load_workspace(&profile_id("main")).await.unwrap(),
        newer
    );
    let failure = mixin
        .save_mixin(
            ProfileRuntimeAccess::<dyn ActiveProfileReferenceApplier>::Offline,
            &source,
            "mode: global\n",
        )
        .await
        .unwrap_err();
    assert_eq!(failure.code, ErrorCode::NotReady);
    assert_eq!(
        profiles.load_workspace(&profile_id("main")).await.unwrap(),
        newer
    );
    assert!(store.sidecars.lock().unwrap().is_empty());
}

#[tokio::test]
async fn shared_editor_rejects_unit_and_old_terminal_keeps_draft_and_accepts_only_the_actual_retry_receipt()
 {
    let store = Arc::new(FakeStore::with_profile("main", "mode: rule\n", true));
    let profiles = profiles_with_mihomo_codec(store.clone());
    let document = ProfileDocumentApplication::new(profiles.clone());
    let observed = document.load(None).await.unwrap();
    let source = observed.workspace_identity.clone();
    let mut session = ProfileEditSession::default();
    assert_eq!(
        session.observe(&source, &observed.content, ""),
        ProfileEditObservation::Updated
    );
    let old = session
        .begin_document(
            "mode: global\n".into(),
            ProfileWriteAuthorization::RequireUnlock,
        )
        .unwrap();
    assert!(!session.can_edit());
    assert_eq!(
        session.finish(old.operation, &old.intent, Ok(CommandOutput::Unit)),
        ProfileEditFinish::Applied
    );
    assert_eq!(
        match &session.operation {
            ProfileEditOperationPhase::Failed(failure) => failure.code.clone(),
            ProfileEditOperationPhase::Ready
            | ProfileEditOperationPhase::Pending(_)
            | ProfileEditOperationPhase::Saved => {
                panic!("invalid editor output must be a typed operation failure")
            }
        },
        ErrorCode::InvalidState
    );
    match session.status_fact("mode: global\n") {
        ProfileEditStatusFact::SaveFailed(failure) => {
            assert_eq!(failure.code, ErrorCode::InvalidState);
            assert!(!failure.message.is_empty());
        }
        other => panic!("expected raw save failure facts, got {other:?}"),
    }
    assert_eq!(store.load("main").await.unwrap(), observed.content);
    assert!(session.dirty("mode: global\n"));
    let retry = session
        .begin_document(
            "mode: global\n".into(),
            ProfileWriteAuthorization::RequireUnlock,
        )
        .unwrap();
    let committed = document
        .save(
            ProfileRuntimeAccess::<dyn ActiveProfileReferenceApplier>::Offline,
            &source,
            "mode: global\n",
            ProfileWriteAuthorization::RequireUnlock,
        )
        .await
        .unwrap();
    let output = CommandOutput::ProfileDocumentSaved(Box::new(ProfileDocumentSaved {
        previous: source,
        document: committed.clone(),
    }));
    assert_eq!(
        session.finish(old.operation, &old.intent, Ok(output.clone())),
        ProfileEditFinish::Stale
    );
    assert!(matches!(
        &session.operation,
        ProfileEditOperationPhase::Pending(_)
    ));
    assert_eq!(
        session.finish(retry.operation, &retry.intent, Ok(output)),
        ProfileEditFinish::Applied
    );
    assert_eq!(session.source(), Some(&committed.workspace_identity));
    assert!(matches!(
        &session.operation,
        ProfileEditOperationPhase::Saved
    ));
    assert!(!session.dirty("mode: global\n"));
    let writes_before_discard = profiles.load_workspace(&profile_id("main")).await.unwrap();
    assert_eq!(session.discard().as_deref(), Some("mode: global\n"));
    assert_eq!(
        profiles.load_workspace(&profile_id("main")).await.unwrap(),
        writes_before_discard
    );
}

#[tokio::test]
async fn a_new_external_source_cannot_inherit_this_editors_prior_saved_claim() {
    let store = Arc::new(FakeStore::with_profile("main", "mode: rule\n", true));
    let profiles = profiles_with_mihomo_codec(store.clone());
    let document = ProfileDocumentApplication::new(profiles);
    let observed = document.load(None).await.unwrap();
    let source = observed.workspace_identity.clone();
    let mut session = ProfileEditSession::default();
    session.observe(&source, &observed.content, "");
    let pending = session
        .begin_document(
            "mode: global\n".into(),
            ProfileWriteAuthorization::RequireUnlock,
        )
        .unwrap();
    let saved = document
        .save(
            ProfileRuntimeAccess::<dyn ActiveProfileReferenceApplier>::Offline,
            &source,
            "mode: global\n",
            ProfileWriteAuthorization::RequireUnlock,
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
    assert!(matches!(
        &session.operation,
        ProfileEditOperationPhase::Saved
    ));
    store.replace_document_for_test("main", "# external editor\nmode: direct\n");
    let newer = document.load(None).await.unwrap();
    assert_eq!(
        session.observe(&newer.workspace_identity, &newer.content, &saved.content),
        ProfileEditObservation::Updated
    );
    assert_eq!(session.source(), Some(&newer.workspace_identity));
    assert!(
        matches!(&session.operation, ProfileEditOperationPhase::Ready),
        "an external write is a read observation, never this edit operation's success"
    );
    assert!(!session.dirty(&newer.content));
}
