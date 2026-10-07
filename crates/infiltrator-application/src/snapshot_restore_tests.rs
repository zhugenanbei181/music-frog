//! test-intent: behavior
//! Real port effects prove frozen review, cancellation, permissions and source comparison.
use super::*;
fn fixture() -> (
    SnapshotApplication,
    Arc<FakeProfileStore>,
    Arc<FakeSnapshotStore>,
) {
    let profiles = Arc::new(FakeProfileStore::with_profile(
        "main",
        "mode: global\nproxies: []\n",
    ));
    let snapshots = Arc::new(FakeSnapshotStore::with_snapshot(
        "main",
        1_750_000_000_000,
        "mode: rule\nproxies: []\n",
    ));
    (
        SnapshotApplication::new(profiles.clone(), snapshots.clone()),
        profiles,
        snapshots,
    )
}
async fn target(app: &SnapshotApplication) -> SnapshotRestoreTarget {
    SnapshotRestoreTarget {
        profile: "main".into(),
        snapshot_id: app.list("main").await.unwrap()[0]
            .path
            .to_string_lossy()
            .into_owned(),
    }
}
#[tokio::test]
async fn preparation_and_cancel_write_nothing_and_cancelled_identity_cannot_be_confirmed() {
    let (app, profiles, _) = fixture();
    let review = app.prepare_restore(&target(&app).await).await.unwrap();
    assert_eq!(review.before_yaml, "mode: global\nproxies: []\n");
    assert_eq!(review.restored_yaml, "mode: rule\nproxies: []\n");
    assert_eq!(profiles.writes.load(Ordering::SeqCst), 0);
    let other = fixture().0;
    assert!(
        other
            .confirm_restore(None::<Arc<dyn ManagedRuntime>>, &review.identity)
            .await
            .is_err()
    );
    app.cancel_restore(&review.identity).unwrap();
    assert!(
        app.confirm_restore(None::<Arc<dyn ManagedRuntime>>, &review.identity)
            .await
            .is_err()
    );
    assert_eq!(profiles.writes.load(Ordering::SeqCst), 0);
    assert_eq!(profiles.load("main").await.unwrap(), review.before_yaml);
}
#[tokio::test]
async fn permission_retry_keeps_review_and_commits_exactly_once() {
    let (app, profiles, _) = fixture();
    let review = app.prepare_restore(&target(&app).await).await.unwrap();
    profiles.denied.store(true, Ordering::SeqCst);
    assert_eq!(
        app.confirm_restore(None::<Arc<dyn ManagedRuntime>>, &review.identity)
            .await
            .unwrap_err()
            .code,
        ErrorCode::Permission
    );
    assert_eq!(profiles.writes.load(Ordering::SeqCst), 0);
    profiles.denied.store(false, Ordering::SeqCst);
    let receipt = app
        .confirm_restore(None::<Arc<dyn ManagedRuntime>>, &review.identity)
        .await
        .unwrap();
    receipt.validate(&review.identity).unwrap();
    assert_eq!(profiles.load("main").await.unwrap(), review.restored_yaml);
    assert_eq!(profiles.writes.load(Ordering::SeqCst), 1);
    assert!(
        app.confirm_restore(None::<Arc<dyn ManagedRuntime>>, &review.identity)
            .await
            .is_err()
    );
    assert_eq!(profiles.writes.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn changed_profile_or_snapshot_rejects_restore_without_overwriting_later_edits() {
    let (app, profiles, _) = fixture();
    let review = app.prepare_restore(&target(&app).await).await.unwrap();
    profiles
        .save("main", "# later user edit\nmode: direct\n")
        .await
        .unwrap();
    let before = profiles.writes.load(Ordering::SeqCst);
    assert_eq!(
        app.confirm_restore(None::<Arc<dyn ManagedRuntime>>, &review.identity)
            .await
            .unwrap_err()
            .code,
        ErrorCode::NotReady
    );
    assert_eq!(profiles.writes.load(Ordering::SeqCst), before);
    assert_eq!(
        profiles.load("main").await.unwrap(),
        "# later user edit\nmode: direct\n"
    );
    app.cancel_restore(&review.identity).unwrap();
    let (app, profiles, snapshots) = fixture();
    let review = app.prepare_restore(&target(&app).await).await.unwrap();
    snapshots.snapshots.lock().unwrap().get_mut("main").unwrap()[0].1 = "mode: direct\n".into();
    assert_eq!(
        app.confirm_restore(None::<Arc<dyn ManagedRuntime>>, &review.identity)
            .await
            .unwrap_err()
            .code,
        ErrorCode::NotReady
    );
    assert_eq!(profiles.writes.load(Ordering::SeqCst), 0);
}
