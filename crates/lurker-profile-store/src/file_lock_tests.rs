use super::lock_profile_writes_for_dir;
use crate::layout::ProfileStoreLayout;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;

#[tokio::test]
async fn managers_for_one_canonical_directory_share_the_write_fence() {
    let directory = TempDir::new().unwrap();
    let layout = ProfileStoreLayout::default();
    let first = lock_profile_writes_for_dir(&layout, directory.path())
        .await
        .unwrap();
    let layout = layout.clone();
    let second_directory = directory.path().to_path_buf();
    let mut second =
        tokio::spawn(async move { lock_profile_writes_for_dir(&layout, &second_directory).await });

    assert!(
        timeout(Duration::from_millis(50), &mut second)
            .await
            .is_err()
    );
    drop(first);
    let acquired = timeout(Duration::from_secs(2), second)
        .await
        .expect("the second manager acquires after the first releases")
        .expect("lock task completes");
    assert!(acquired.is_ok());
}
