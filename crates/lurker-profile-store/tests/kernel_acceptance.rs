//! Neutral kernel acceptance tests: write-fence contention, atomic replacement
//! and profile-state backup recovery.
//!
//! These exercise the persistence primitives every product facade delegates to,
//! with no product type, engine driver or credential involved.

use lurker_profile_store::document_store::{
    read_optional, write_document, write_document_bytes_durable,
};
use lurker_profile_store::durable_replace::{DurableReplaceError, replace_durable};
use lurker_profile_store::file_lock::lock_profile_writes_for_dir;
use lurker_profile_store::instance_lock::try_acquire_instance_lock;
use lurker_profile_store::layout::ProfileStoreLayout;
use lurker_profile_store::namespace::{
    read_profile_state, recover_profile_state_backup, write_profile_state_atomically,
};
use std::ffi::OsString;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;

fn layout() -> ProfileStoreLayout {
    ProfileStoreLayout::default()
}

#[tokio::test]
async fn write_fence_admits_one_holder_and_serializes_a_read_modify_write() {
    let directory = TempDir::new().unwrap();
    let layout = Arc::new(layout());
    let counter = directory.path().join("counter.txt");
    write_document(&counter, "0").await.unwrap();

    let first = lock_profile_writes_for_dir(&layout, directory.path())
        .await
        .unwrap();

    // A second holder may not enter while the first lease is alive.
    let contender_dir = directory.path().to_path_buf();
    let contender_layout = Arc::clone(&layout);
    let mut contender = tokio::spawn(async move {
        lock_profile_writes_for_dir(&contender_layout, &contender_dir).await
    });
    assert!(
        timeout(Duration::from_millis(50), &mut contender)
            .await
            .is_err(),
        "a contended write fence must not be granted twice"
    );
    drop(first);
    let granted = timeout(Duration::from_secs(2), contender)
        .await
        .expect("the waiting holder is granted after release")
        .expect("lock task completes");
    assert!(granted.is_ok());
    drop(granted);

    // Every writer takes the fence, so increments cannot interleave.
    let mut writers = Vec::new();
    for _ in 0..8 {
        let layout = Arc::clone(&layout);
        let directory = directory.path().to_path_buf();
        let counter = counter.clone();
        writers.push(tokio::spawn(async move {
            let _guard = lock_profile_writes_for_dir(&layout, &directory)
                .await
                .unwrap();
            let current = read_optional(&counter).await.unwrap().unwrap();
            let next: u32 = current.parse::<u32>().unwrap() + 1;
            write_document(&counter, &next.to_string()).await.unwrap();
        }));
    }
    for writer in writers {
        writer.await.unwrap();
    }
    assert_eq!(read_optional(&counter).await.unwrap().unwrap(), "8");
}

#[tokio::test]
async fn cross_process_lease_has_exactly_one_winner() {
    let directory = TempDir::new().unwrap();
    let lock_path = directory.path().join(".profile-write-test.lock");

    let winner = try_acquire_instance_lock(&lock_path).unwrap();
    assert!(winner.is_some(), "the first lease acquisition must win");
    let loser = try_acquire_instance_lock(&lock_path).unwrap();
    assert!(
        loser.is_none(),
        "a held kernel lease must be rejected instead of shared"
    );
    drop(winner);
    assert!(
        try_acquire_instance_lock(&lock_path).unwrap().is_some(),
        "the lease is reacquirable after the winner releases it"
    );
}

#[tokio::test]
async fn atomic_replace_publishes_a_complete_value_and_leaves_no_temporary() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("state.toml");

    let first = "x".repeat(64 * 1024);
    write_document(&path, &first).await.unwrap();
    let second = "y".repeat(128 * 1024);
    write_document(&path, &second).await.unwrap();

    assert_eq!(fs::read_to_string(&path).unwrap(), second);
    let entries = fs::read_dir(directory.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect::<Vec<_>>();
    assert_eq!(entries, vec![OsString::from("state.toml")]);
}

#[tokio::test]
async fn failed_replacement_preserves_the_previous_value() {
    let directory = TempDir::new().unwrap();
    let other = TempDir::new().unwrap();
    let destination = directory.path().join("state.toml");
    write_document(&destination, "committed").await.unwrap();

    // A staged file outside the destination directory violates the same-filesystem
    // rule, so the replacement must fail before touching the destination.
    let staged = other.path().join("staged.toml");
    fs::write(&staged, b"uncommitted").unwrap();
    match replace_durable(&staged, &destination) {
        Err(DurableReplaceError::NotCommitted(_)) => {}
        other => panic!("expected a pre-commit failure, got {other:?}"),
    }
    assert_eq!(fs::read_to_string(&destination).unwrap(), "committed");
    assert!(
        staged.exists(),
        "the staged copy is never deleted by the callee"
    );
}

#[tokio::test]
async fn missing_profile_state_is_recovered_from_its_backup() {
    let directory = TempDir::new().unwrap();
    let layout = layout();
    let state_path = directory.path().join("profile-state.toml");
    let backup = layout.profile_state_backup_path(&state_path);
    let previous = "[default]\nprofile = \"primary\"\n";
    fs::write(&backup, previous).unwrap();

    recover_profile_state_backup(&layout, &state_path)
        .await
        .unwrap();

    assert_eq!(fs::read_to_string(&state_path).unwrap(), previous);
    assert!(!backup.exists(), "a replayed backup is retired");
}

#[tokio::test]
async fn state_replacement_keeps_a_complete_document() {
    let directory = TempDir::new().unwrap();
    let layout = layout();
    let state_path = directory.path().join("profile-state.toml");
    let state: toml::Value = toml::from_str("[profiles.main]\nname = \"main\"\n").unwrap();

    write_profile_state_atomically(&layout, &state_path, &state)
        .await
        .unwrap();

    let observed = read_profile_state(&layout, &state_path).await.unwrap();
    assert_eq!(observed, state);
    assert!(
        !layout.profile_state_backup_path(&state_path).exists(),
        "a committed state write leaves no recovery artifact"
    );
}

#[tokio::test]
async fn document_bytes_round_trip_through_the_durable_path() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("nested/opaque.bin");
    write_document_bytes_durable(&path, &[0, 1, 2, 255])
        .await
        .unwrap();

    assert_eq!(fs::read(&path).unwrap(), vec![0, 1, 2, 255]);
    assert!(Path::new(&path).exists());
}
