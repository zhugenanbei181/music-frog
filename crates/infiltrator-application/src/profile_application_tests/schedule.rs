//! Behavior cases for schedule.
//! test-intent: behavior

use super::*;

/// DUAL-07-14: the shared schedule draft validates and persists URL,
/// auto-update, interval, and cron in one application call, so neither surface
/// assembles profile metadata itself.
#[tokio::test]
async fn schedule_draft_validates_and_persists_the_subscription_shape() {
    let store = subscription_store(None, None, None, false).await;
    let application = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>);

    // Happy path: an interval-based schedule with a cron cadence on top.
    application
        .update_subscription_schedule(
            "main",
            &SubscriptionScheduleDraft {
                url: "https://sub.example.com/token".to_string(),
                auto_update_enabled: true,
                update_interval_hours: "12".to_string(),
                cron_expression: Some("0 */6 * * *".to_string()),
            },
        )
        .await
        .expect("valid draft persists");
    let metadata = application.load_metadata("main").await.expect("metadata");
    assert_eq!(
        metadata.subscription_url.as_deref(),
        Some("https://sub.example.com/token")
    );
    assert!(metadata.auto_update_enabled);
    assert_eq!(metadata.update_interval_hours, Some(12));
    assert_eq!(metadata.cron_expression.as_deref(), Some("0 */6 * * *"));
    assert!(
        metadata.next_update.is_none(),
        "schedule recomputes on update"
    );

    // An interval-less draft with cron is a cron-only schedule.
    application
        .update_subscription_schedule(
            "main",
            &SubscriptionScheduleDraft {
                url: "https://sub.example.com/token".to_string(),
                auto_update_enabled: true,
                update_interval_hours: String::new(),
                cron_expression: Some("@daily".to_string()),
            },
        )
        .await
        .expect("cron-only draft persists");
    let metadata = application.load_metadata("main").await.expect("metadata");
    assert_eq!(metadata.update_interval_hours, None);
    assert_eq!(
        metadata.cron_expression.as_deref(),
        Some("0 0 * * *"),
        "a macro schedule is stored in its normalized five-field form"
    );

    // A draft without a cron falls back to the 24h default.
    application
        .update_subscription_schedule(
            "main",
            &SubscriptionScheduleDraft {
                url: "https://sub.example.com/token".to_string(),
                auto_update_enabled: true,
                update_interval_hours: String::new(),
                cron_expression: None,
            },
        )
        .await
        .expect("default interval draft persists");
    let metadata = application.load_metadata("main").await.expect("metadata");
    assert_eq!(metadata.update_interval_hours, Some(24));

    // Typed rejections: malformed cron / zero interval / auto-update without URL.
    for draft in [
        SubscriptionScheduleDraft {
            url: "https://x".to_string(),
            auto_update_enabled: true,
            update_interval_hours: "24".to_string(),
            cron_expression: Some("not a cron".to_string()),
        },
        SubscriptionScheduleDraft {
            url: "https://x".to_string(),
            auto_update_enabled: true,
            update_interval_hours: "0".to_string(),
            cron_expression: None,
        },
        SubscriptionScheduleDraft {
            url: "https://x".to_string(),
            auto_update_enabled: true,
            update_interval_hours: "not-a-number".to_string(),
            cron_expression: None,
        },
        SubscriptionScheduleDraft {
            url: "  ".to_string(),
            auto_update_enabled: true,
            update_interval_hours: "24".to_string(),
            cron_expression: None,
        },
    ] {
        let failure = application
            .update_subscription_schedule("main", &draft)
            .await
            .expect_err("invalid draft is rejected");
        assert_eq!(failure.code, ErrorCode::InvalidInput);
    }

    // Clearing the URL clears the whole schedule.
    application
        .update_subscription_schedule("main", &SubscriptionScheduleDraft::default())
        .await
        .expect("clearing persists");
    let metadata = application.load_metadata("main").await.expect("metadata");
    assert!(metadata.subscription_url.is_none());
    assert!(!metadata.auto_update_enabled);
    assert!(metadata.update_interval_hours.is_none());
    assert!(metadata.cron_expression.is_none());
    assert!(metadata.last_updated.is_none());
}
