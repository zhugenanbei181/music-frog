use crate::active_reference_file::ActiveProfileReferenceFile;
use crate::active_reference_owner::ActiveProfileReferenceRecord;
use crate::document_store::RECORD_DURABILITY_UNCERTAIN_PREFIX;
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReferenceRevision, ActiveProfileReferenceState,
};
use infiltrator_ports::error::PortError;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tempfile::TempDir;
use tokio::fs;
use tokio::time::timeout;

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
struct ReferenceRecord {
    revision: u64,
    selected: Option<String>,
}

#[tokio::test]
async fn product_reference_file_reads_publishes_and_reads_back_typed_records() {
    let root = TempDir::new().unwrap();
    let file = ActiveProfileReferenceFile::new(root.path());
    let record = ReferenceRecord {
        revision: 7,
        selected: Some("mihomo/main".into()),
    };

    assert!(file.read::<ReferenceRecord>().await.unwrap().is_none());
    assert_eq!(file.publish(&record).await.unwrap(), record);
    assert_eq!(file.read::<ReferenceRecord>().await.unwrap(), Some(record));
}

#[tokio::test]
async fn invalid_reference_json_fails_closed() {
    let root = TempDir::new().unwrap();
    let file = ActiveProfileReferenceFile::new(root.path());
    fs::write(file.path(), b"{broken").await.unwrap();

    assert!(file.read::<ReferenceRecord>().await.is_err());
}

#[test]
fn synchronous_owner_read_and_write_use_the_same_record_path() {
    let root = TempDir::new().unwrap();
    let file = ActiveProfileReferenceFile::new(root.path());
    let record = ReferenceRecord {
        revision: 9,
        selected: Some("sing-box/profile".into()),
    };

    assert!(file.read_sync::<ReferenceRecord>().unwrap().is_none());
    file.write_sync(&record).unwrap();
    assert_eq!(file.read_sync::<ReferenceRecord>().unwrap(), Some(record));
}

#[test]
fn durable_record_publication_reconciles_visible_state_after_durability_error() {
    let expected = ActiveProfileReferenceRecord::pending_legacy_migration();
    let write_error = PortError::Failed(format!(
        "{RECORD_DURABILITY_UNCERTAIN_PREFIX}: simulated directory flush failure"
    ));

    let result =
        super::reconcile_record_publication("test record", &expected, Err(write_error), || {
            Ok(expected.clone())
        });

    assert!(result.is_ok());
}

#[test]
fn durable_record_publication_rejects_mismatched_readback_after_durability_error() {
    let expected = ActiveProfileReferenceRecord::pending_legacy_migration();
    let observed = ActiveProfileReferenceRecord {
        selection: ActiveProfileReferenceState::Unselected {
            revision: ActiveProfileReferenceRevision::new(1),
        },
        ..expected.clone()
    };
    let write_error = PortError::Failed(format!(
        "{RECORD_DURABILITY_UNCERTAIN_PREFIX}: simulated directory flush failure"
    ));

    let result =
        super::reconcile_record_publication("test record", &expected, Err(write_error), || {
            Ok(observed)
        });

    assert!(matches!(result, Err(PortError::Failed(_))));
}

#[tokio::test]
async fn product_root_lock_serializes_independent_file_owners() {
    let root = TempDir::new().unwrap();
    let first = ActiveProfileReferenceFile::new(root.path());
    let second = ActiveProfileReferenceFile::new(root.path());
    let (_, first_guard) = timeout(Duration::from_secs(2), first.acquire_root_lock())
        .await
        .unwrap()
        .unwrap();

    assert!(
        timeout(Duration::from_millis(60), second.acquire_root_lock())
            .await
            .is_err()
    );

    drop(first_guard);
    let (_, second_guard) = timeout(Duration::from_secs(2), second.acquire_root_lock())
        .await
        .unwrap()
        .unwrap();
    drop(second_guard);
}

#[tokio::test]
async fn product_reference_and_owner_locks_are_distinct_and_orderable() {
    let root = TempDir::new().unwrap();
    let file = ActiveProfileReferenceFile::new(root.path());
    let (canonical_root, root_guard) = file.acquire_root_lock().await.unwrap();
    let canonical_file = ActiveProfileReferenceFile::new(canonical_root);
    let owner_state_guard = canonical_file.acquire_owner_state_lock().await.unwrap();
    let owner_guard = canonical_file
        .try_acquire_owner_lock()
        .unwrap()
        .expect("owner admission lock is initially free");

    assert!(canonical_file.try_acquire_owner_lock().unwrap().is_none());
    assert!(
        timeout(
            Duration::from_millis(60),
            canonical_file.acquire_owner_state_lock(),
        )
        .await
        .is_err()
    );

    drop(owner_guard);
    drop(owner_state_guard);
    drop(root_guard);
}
