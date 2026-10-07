//! Behavior cases for successful.
//! test-intent: behavior

use super::*;
use infiltrator_domain::subscription_scheduler_policy::SubscriptionSchedule;

/// DUAL-07-03: a successful update on a cron-only profile advances
/// `next_update` to the Cron expression's next occurrence, not to an interval.
#[tokio::test]
async fn successful_cron_update_advances_to_the_next_occurrence() {
    let store = subscription_store(None, None, None, false).await;
    {
        let mut profiles = store.profiles.lock().expect("profiles lock");
        let metadata = &mut profiles.get_mut("main").expect("profile").1;
        metadata.update_interval_hours = None;
        metadata.cron_expression = Some("0 */6 * * *".to_string());
    }
    let application = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>);
    let source = FakeSource::modified("proxies:\n  - name: a\n    type: ss\n", None);

    application
        .update_subscription_conditional(&source, "main")
        .await
        .expect("update");

    let metadata = store.get_profile_metadata("main").await.expect("metadata");
    let next = metadata
        .next_update
        .expect("cron schedule must set next_update");
    let now = Utc::now();
    assert!(next > now, "next run must be in the future");
    let schedule =
        SubscriptionSchedule::from_metadata(None, Some("0 */6 * * *")).expect("schedule");
    assert_eq!(
        schedule.next_run(now),
        Some(next),
        "next_update must be the cron occurrence after now"
    );
    assert_eq!(next.minute(), 0);
    assert_eq!(next.hour() % 6, 0);
}
