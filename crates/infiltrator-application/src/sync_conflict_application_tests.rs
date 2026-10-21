//! Behaviour tests for the WebDAV conflict-resolution use case.
//!
//! The fake sync owner and the fake profile owner prove the three paths the
//! product depends on: a resolution that really moves the owner's state, a
//! typed refusal that never claims a resolution, and a stale answer that
//! cannot move the state of the conflict the owner moved on to.

use super::*;
use crate::sync_conflict_test_support::{FakeProfileStore, FakeSyncPort};
use infiltrator_ports::sync::SyncPort;
use std::sync::Arc;

fn application(port: Arc<FakeSyncPort>, store: Arc<FakeProfileStore>) -> SyncConflictApplication {
    SyncConflictApplication::new(
        SyncApplication::new(port),
        ProfileApplication::new(store),
        ProfileRuntimeAccess::Offline,
        Some("/fake/configs".to_owned()),
    )
}

#[tokio::test]
async fn keeping_the_local_configuration_discards_the_remote_copy() {
    let port = Arc::new(FakeSyncPort::with_conflict("mode: rule\n"));
    let store = Arc::new(FakeProfileStore::with_content("mode: direct\n"));
    let receipt = application(Arc::clone(&port), Arc::clone(&store))
        .resolve(SyncConflictResolution::KeepLocal)
        .await
        .expect("the owner resolves its conflict");

    assert_eq!(receipt.resolution, SyncConflictResolution::KeepLocal);
    assert_eq!(receipt.conflict.profile, "main");
    // The owner's state really changed: the copy is discarded and the owner
    // holds no conflict any more.
    assert!(port.discarded());
    assert!(port.current_conflict().await.expect("owner read").is_none());
    // The local configuration stayed the truth.
    assert_eq!(store.content(), "mode: direct\n");
    assert_eq!(store.saves(), 0);
}

#[tokio::test]
async fn taking_the_remote_configuration_writes_the_local_profile() {
    let port = Arc::new(FakeSyncPort::with_conflict("mode: rule\n"));
    let store = Arc::new(FakeProfileStore::with_content("mode: direct\n"));
    let receipt = application(Arc::clone(&port), Arc::clone(&store))
        .resolve(SyncConflictResolution::TakeRemote)
        .await
        .expect("the owner resolves its conflict");

    assert_eq!(receipt.resolution, SyncConflictResolution::TakeRemote);
    assert_eq!(receipt.conflict.profile, "main");
    // The local profile really holds the remote content ...
    assert_eq!(store.content(), "mode: rule\n");
    assert_eq!(store.saves(), 1);
    // ... and the remote copy is discarded afterwards.
    assert!(port.discarded());
    assert!(port.current_conflict().await.expect("owner read").is_none());
}

#[tokio::test]
async fn a_resolution_without_a_pending_conflict_is_refused_with_a_typed_failure() {
    let port = Arc::new(FakeSyncPort::without_conflict());
    let store = Arc::new(FakeProfileStore::with_content("mode: direct\n"));
    let failure = application(Arc::clone(&port), Arc::clone(&store))
        .resolve(SyncConflictResolution::KeepLocal)
        .await
        .expect_err("no conflict is pending");

    assert_eq!(failure.code, ErrorCode::InvalidState);
    assert!(!failure.retryable);
    assert_eq!(store.saves(), 0);
    assert!(!port.discarded());
}

#[tokio::test]
async fn an_invalid_remote_configuration_keeps_the_conflict_copy() {
    let port = Arc::new(FakeSyncPort::with_conflict("port: 7890\nmode: : rule\n"));
    let store = Arc::new(FakeProfileStore::with_content("mode: direct\n"));
    let failure = application(Arc::clone(&port), Arc::clone(&store))
        .resolve(SyncConflictResolution::TakeRemote)
        .await
        .expect_err("the remote copy is not a configuration");

    assert_eq!(failure.code, ErrorCode::Configuration);
    // The local profile kept its content and the remote copy survived, so the
    // conflict can still be resolved.
    assert_eq!(store.content(), "mode: direct\n");
    assert_eq!(store.saves(), 0);
    assert!(!port.discarded());
}

#[tokio::test]
async fn an_owner_that_keeps_the_conflict_reports_a_typed_refusal() {
    let port = Arc::new(FakeSyncPort::with_conflict("mode: rule\n"));
    port.keeping_the_conflict_after_discard();
    let store = Arc::new(FakeProfileStore::with_content("mode: direct\n"));
    let failure = application(Arc::clone(&port), Arc::clone(&store))
        .resolve(SyncConflictResolution::KeepLocal)
        .await
        .expect_err("the conflict was not cleared");

    assert_eq!(failure.code, ErrorCode::InvalidState);
    assert!(!failure.retryable);
}

#[tokio::test]
async fn a_stale_answer_cannot_move_the_state_of_the_next_conflict() {
    let port = Arc::new(FakeSyncPort::with_conflict("mode: rule\n"));
    port.publishing_successor(SyncConflict {
        profile: "other".to_owned(),
        remote_path: "/fake/configs/other.remote-conflict.yaml".to_owned(),
    });
    let store = Arc::new(FakeProfileStore::with_content("mode: direct\n"));
    let failure = application(Arc::clone(&port), Arc::clone(&store))
        .resolve(SyncConflictResolution::KeepLocal)
        .await
        .expect_err("the owner moved on to another conflict");

    assert_eq!(failure.code, ErrorCode::NotReady);
    assert!(failure.retryable);
    // The successor conflict is still the owner's pending conflict: the stale
    // answer did not resolve it.
    let observed = port.current_conflict().await.expect("owner read");
    assert_eq!(
        observed.map(|conflict| conflict.conflict.profile),
        Some("other".to_owned())
    );
}
