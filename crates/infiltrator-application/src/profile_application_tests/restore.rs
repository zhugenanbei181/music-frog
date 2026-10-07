//! Behavior cases for restore.
//! test-intent: behavior

use super::*;

#[tokio::test]
async fn restore_backup_reports_availability_and_is_one_shot() {
    let store = Arc::new(FakeStore::with_profile("main", "proxies: []\n", true));
    store
        .restorable_backups
        .lock()
        .expect("backup lock")
        .push("main".to_string());
    let application = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>);

    let listed = application.list_profiles().await.expect("list");
    assert!(listed[0].has_backup, "backup presence is projected");

    assert!(
        application.restore_backup("main").await.expect("restore"),
        "an existing backup restores"
    );
    assert!(
        !application.restore_backup("main").await.expect("restore"),
        "a consumed backup is not restored twice"
    );
    let listed = application.list_profiles().await.expect("list");
    assert!(!listed[0].has_backup, "restore clears the projected flag");
}
