//! test-intent: behavior
use super::*;
use infiltrator_domain::rules::source_identity::identify_rules_document;
use tokio::time::timeout;

async fn start_ready(fixture: &Fixture) {
    let generation = fixture.session.start().await.unwrap();
    fixture
        .session
        .wait_for_ready(generation, Duration::from_secs(5))
        .await
        .unwrap();
}

struct ConcurrentEdit<'a> {
    config: &'a ConfigManager<MockStore>,
}
#[async_trait]
impl PreparedConfigReload for ConcurrentEdit<'_> {
    async fn reload(&self, _: &Path) -> Result<(), String> {
        write_profile_file_outside_store_for_test(
            self.config,
            "main",
            "port: 7893\n# newer edit\n",
        )
        .await;
        Err("reload rejected after concurrent edit".into())
    }
}

#[async_trait]
impl ConfigReloader for ConcurrentEdit<'_> {
    async fn prepare(&self) -> Result<Box<dyn PreparedConfigReload + '_>, String> {
        Ok(Box::new(self))
    }
}

#[tokio::test]
async fn failed_apply_releases_persistence_boundary_and_never_rolls_back_over_a_newer_edit() {
    let f = fixture(0, false).await;
    let generation = f.session.start().await.unwrap();
    f.session
        .wait_for_ready(generation, Duration::from_secs(5))
        .await
        .unwrap();
    f.controller.fail_starts_left.store(1, Ordering::SeqCst);
    let expected = identify_rules_document("main".into(), OLD);
    let reloader = ConcurrentEdit { config: &f.config };
    let failure = timeout(
        Duration::from_secs(2),
        apply_confirmed_to_selected_reference(
            &f.session,
            &f.config,
            &reloader,
            NEW,
            params(ApplyStrategy::PreferReload),
            &expected,
        ),
    )
    .await
    .expect("reload can acquire the persistence boundary")
    .unwrap_err();
    match failure {
        ApplyError::RollbackFailed { cause, rollback } => {
            assert!(cause.contains("start rejected"));
            assert!(rollback.contains("source profile changed"));
        }
        other => panic!("newer edit must not be silently restored away: {other:?}"),
    }
    assert_eq!(
        f.config.load("main").await.unwrap(),
        "port: 7893\n# newer edit\n"
    );
    assert_eq!(
        f.config.apply_transaction("main").unwrap().stage,
        ApplyTransactionStage::RollbackFailed
    );
}

#[tokio::test]
async fn independent_runtime_managers_never_borrow_another_products_apply_receipt() {
    let first = fixture(0, false).await;
    let second = fixture(0, false).await;
    start_ready(&first).await;
    start_ready(&second).await;
    assert!(first.config.apply_transaction("main").is_none());
    assert!(second.config.apply_transaction("main").is_none());
    apply_to_selected_reference(
        &first.session,
        &first.config,
        &first.reloader,
        NEW,
        params(ApplyStrategy::PreferReload),
    )
    .await
    .unwrap();
    assert_eq!(
        first.config.apply_transaction("main").unwrap().stage,
        ApplyTransactionStage::Committed
    );
    assert!(
        second.config.apply_transaction("main").is_none(),
        "same profile name cannot identify another product's transaction"
    );
    assert!(
        first.config.apply_transaction("other").is_none(),
        "unrelated profiles have separate observations"
    );
    apply_to_selected_reference(
        &second.session,
        &second.config,
        &second.reloader,
        "port: 7892\n",
        params(ApplyStrategy::PreferReload),
    )
    .await
    .unwrap();
    assert_eq!(first.config.load("main").await.unwrap(), NEW);
    assert_eq!(second.config.load("main").await.unwrap(), "port: 7892\n");
    assert_eq!(
        first.config.apply_transaction("main").unwrap().stage,
        ApplyTransactionStage::Committed
    );
    assert_eq!(
        second.config.apply_transaction("main").unwrap().stage,
        ApplyTransactionStage::Committed
    );
}
