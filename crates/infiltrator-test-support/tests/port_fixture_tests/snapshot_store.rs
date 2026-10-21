//! Behaviour of the isolated snapshot-store port fixture.

use infiltrator_ports::snapshot_store::SnapshotStore;
use infiltrator_test_support::snapshot_store_fixtures::{CAPTURE_PROFILE, SnapshotStoreCapture};
use lurker_core_ports::error::PortError;
use std::path::PathBuf;

#[tokio::test]
async fn saving_publishes_the_real_metadata() {
    let store = SnapshotStoreCapture::new();

    let meta = store
        .save(CAPTURE_PROFILE, "mixed-port: 7890\n")
        .await
        .expect("saved snapshot");

    assert_eq!(meta.profile, CAPTURE_PROFILE);
    assert_eq!(meta.sha256.len(), 64);
    assert!(meta.path.to_string_lossy().contains(CAPTURE_PROFILE));
    let name = meta
        .path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .expect("snapshot file name");
    assert!(name.ends_with(".yaml"));
}

#[tokio::test]
async fn listing_publishes_the_saved_snapshots() {
    let store = SnapshotStoreCapture::new();
    store
        .save(CAPTURE_PROFILE, "mixed-port: 7890\n")
        .await
        .expect("saved snapshot");
    store
        .save(CAPTURE_PROFILE, "mixed-port: 7891\n")
        .await
        .expect("saved snapshot");

    let listed = store.list(CAPTURE_PROFILE).await.expect("snapshot list");

    assert_eq!(listed.len(), 2);
    assert!(listed[0].timestamp <= listed[1].timestamp);
    assert_ne!(listed[0].sha256, listed[1].sha256);
}

#[tokio::test]
async fn the_saved_content_is_read_back_verbatim() {
    let store = SnapshotStoreCapture::new();
    let meta = store
        .save(CAPTURE_PROFILE, "mixed-port: 7890\n")
        .await
        .expect("saved snapshot");

    let content = store
        .read(CAPTURE_PROFILE, &meta.path)
        .await
        .expect("snapshot content");

    assert_eq!(content, "mixed-port: 7890\n");
}

#[tokio::test]
async fn reading_an_unknown_snapshot_is_not_found() {
    let store = SnapshotStoreCapture::new();
    let path = PathBuf::from(format!(
        "/isolated-snapshots/{CAPTURE_PROFILE}/missing.yaml"
    ));

    let error = store
        .read(CAPTURE_PROFILE, &path)
        .await
        .expect_err("an unknown snapshot must not be found");

    assert!(matches!(error, PortError::NotFound(_)));
}

#[tokio::test]
async fn deleting_outside_the_history_directory_is_refused() {
    let store = SnapshotStoreCapture::new();
    let outside = PathBuf::from("/isolated-snapshots/other-profile/1-abcdef12.yaml");

    let error = store
        .delete(CAPTURE_PROFILE, &outside)
        .await
        .expect_err("a foreign history path must be refused");

    assert!(matches!(error, PortError::Rejected(_)));
}

#[tokio::test]
async fn deleting_a_saved_snapshot_removes_it() {
    let store = SnapshotStoreCapture::new();
    let meta = store
        .save(CAPTURE_PROFILE, "mixed-port: 7890\n")
        .await
        .expect("saved snapshot");

    store
        .delete(CAPTURE_PROFILE, &meta.path)
        .await
        .expect("delete snapshot");

    assert!(store.list(CAPTURE_PROFILE).await.expect("list").is_empty());
}
