#[cfg(test)]
mod tests {
    use crate::TEST_LOCK;
    use crate::admin_api::state::AdminApiContext;
    use crate::scheduler::subscription::{
        SubscriptionUpdateSummary, schedule_next_attempt, update_all_subscriptions,
    };
    use crate::scheduler::{
        cancel_profile_update_job, schedule_profile_update_job, subscription_job_name,
        subscription_jobs, sync_profile_job,
    };
    use anyhow::anyhow;
    use chrono::{Duration as ChronoDuration, Utc};
    use infiltrator_core::settings::AppSettings;
    use infiltrator_core::subscription::mask_subscription_url;
    use infiltrator_http::HttpClient;
    use mihomo_api::client::MihomoClient;
    use mihomo_config::manager::ConfigManager;
    use mihomo_config::profile::Profile;
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    type Notifications = Arc<Mutex<Vec<(String, bool, Option<String>)>>>;

    #[derive(Clone)]
    struct MockContext {
        notifications: Notifications,
    }

    #[async_trait::async_trait]
    impl AdminApiContext for MockContext {
        async fn notify_subscription_update(
            &self,
            profile: String,
            success: bool,
            message: Option<String>,
        ) {
            self.notifications
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push((profile, success, message));
        }

        async fn rebuild_runtime(&self) -> anyhow::Result<()> {
            Ok(())
        }

        async fn set_use_bundled_core(&self, _enabled: bool) {}
        async fn refresh_core_version_info(&self) {}
        async fn latest_stable_core(&self) -> anyhow::Result<(String, String)> {
            Ok(("v1.20.0".to_string(), "2026-01-01T00:00:00Z".to_string()))
        }
        async fn editor_path(&self) -> Option<String> {
            None
        }
        async fn set_editor_path(&self, _path: Option<String>) {}
        async fn pick_editor_path(&self) -> Option<String> {
            None
        }
        async fn open_profile_in_editor(&self, _profile_name: &str) -> anyhow::Result<()> {
            Ok(())
        }
        async fn get_app_settings(&self) -> AppSettings {
            AppSettings::default()
        }
        async fn save_app_settings(&self, _settings: AppSettings) -> anyhow::Result<()> {
            Ok(())
        }
        async fn runtime_running(&self) -> bool {
            false
        }
        async fn runtime_controller_url(&self) -> Option<String> {
            None
        }
        async fn stop_runtime(&self) -> anyhow::Result<()> {
            Ok(())
        }
        async fn runtime_client(&self) -> anyhow::Result<MihomoClient> {
            Err(anyhow!(
                "runtime client is not available in subscription tests"
            ))
        }
        async fn system_proxy_enabled(&self) -> bool {
            false
        }
        async fn set_system_proxy_enabled(&self, _enabled: bool) -> anyhow::Result<()> {
            Ok(())
        }
        async fn autostart_enabled(&self) -> bool {
            false
        }
        async fn set_autostart_enabled(&self, _enabled: bool) -> anyhow::Result<()> {
            Ok(())
        }
        fn supports_system_proxy_control(&self) -> bool {
            false
        }
        fn supports_autostart_control(&self) -> bool {
            false
        }
    }

    #[tokio::test]
    async fn test_update_subscription_summary() {
        let summary = SubscriptionUpdateSummary {
            total: 5,
            updated: 3,
            failed: 1,
            skipped: 1,
        };

        assert_eq!(summary.total, 5);
        assert_eq!(summary.updated, 3);
        assert_eq!(summary.failed, 1);
        assert_eq!(summary.skipped, 1);
    }

    #[tokio::test]
    async fn test_update_all_subscriptions_with_no_profiles() {
        let _guard = TEST_LOCK.lock().await;
        let temp_dir = tempfile::Builder::new()
            .prefix("sub-test-none-")
            .tempdir()
            .unwrap();
        mihomo_platform::paths::clear_home_dir_override();
        mihomo_platform::paths::set_home_dir_override(temp_dir.path().to_path_buf());

        let ctx = MockContext {
            notifications: Arc::new(Mutex::new(vec![])),
        };
        let client = HttpClient::new();
        let raw_client = HttpClient::new();

        let result = update_all_subscriptions(&ctx, &client, &raw_client).await;

        assert!(result.is_ok());
        let summary = result.unwrap();
        assert!(summary.total <= 1);

        mihomo_platform::paths::clear_home_dir_override();
    }

    #[tokio::test]
    async fn test_update_all_subscriptions_parallel_concurrency() {
        let _guard = TEST_LOCK.lock().await;
        let temp_dir = tempfile::Builder::new()
            .prefix("sub-test-parallel-")
            .tempdir()
            .unwrap();
        mihomo_platform::paths::clear_home_dir_override();
        mihomo_platform::paths::set_home_dir_override(temp_dir.path().to_path_buf());

        let manager = ConfigManager::new().unwrap();

        let ctx = MockContext {
            notifications: Arc::new(Mutex::new(vec![])),
        };
        let client = HttpClient::new();
        let raw_client = HttpClient::new();

        let configs_dir = temp_dir.path().join("configs");
        let _ = std::fs::create_dir_all(&configs_dir);

        for i in 0..10 {
            let profile_name = format!("test-profile-{}", i);
            let profile_path = configs_dir.join(format!("{}.yaml", profile_name));
            let _ = std::fs::write(&profile_path, "port: 7890");

            let mut profile = Profile::new(profile_name.clone(), profile_path, false);
            profile.subscription_url = Some(format!("http://example.com/subscription/{}", i));
            profile.auto_update_enabled = true;
            profile.update_interval_hours = Some(24);

            manager
                .update_profile_metadata(&profile_name, &profile)
                .await
                .unwrap();
        }

        let profiles = manager.list_profiles().await.unwrap();
        assert!(
            profiles.len() >= 10,
            "Manager should see at least 10 profiles, but saw {}",
            profiles.len()
        );

        let result = update_all_subscriptions(&ctx, &client, &raw_client).await;

        assert!(result.is_ok());
        let summary = result.unwrap();
        assert!(
            summary.total >= 10,
            "Summary total should be >= 10, but was {}",
            summary.total
        );

        mihomo_platform::paths::clear_home_dir_override();
    }

    #[tokio::test]
    async fn test_mask_subscription_url_v2() {
        // Updated expectation to match core implementation
        assert_eq!(
            mask_subscription_url("https://example.com/link/abcdefg123456?mu=0"),
            "https://example.com/link/***?mu=0"
        );
    }

    #[tokio::test]
    async fn test_schedule_next_attempt() {
        let _guard = TEST_LOCK.lock().await;
        let temp_dir = tempfile::Builder::new()
            .prefix("sub-test-schedule-")
            .tempdir()
            .unwrap();
        mihomo_platform::paths::clear_home_dir_override();
        mihomo_platform::paths::set_home_dir_override(temp_dir.path().to_path_buf());

        let manager = ConfigManager::new().unwrap();

        let profile_name = "test-schedule".to_string();
        let configs_dir = temp_dir.path().join("configs");
        let _ = std::fs::create_dir_all(&configs_dir);
        let profile_path = configs_dir.join(format!("{}.yaml", profile_name));
        let _ = std::fs::write(&profile_path, "port: 7890");

        let profile = Profile::new(profile_name.clone(), profile_path, false);

        let now = Utc::now();
        let interval_hours = 24u32;

        schedule_next_attempt(&manager, &profile, interval_hours, now)
            .await
            .unwrap();

        let updated_profile = manager.get_profile_metadata(&profile_name).await.unwrap();

        if let Some(next_update) = updated_profile.next_update {
            let expected = now + ChronoDuration::hours(interval_hours as i64);
            assert!(
                next_update >= expected - ChronoDuration::seconds(30)
                    && next_update <= expected + ChronoDuration::seconds(30)
            );
        } else {
            panic!("next_update should be set for profile: {}", profile_name);
        }

        mihomo_platform::paths::clear_home_dir_override();
    }

    /// Wait until the mock context recorded `target` subscription-update
    /// notifications.
    async fn wait_for_notifications(ctx: &MockContext, target: usize) {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let count = ctx
                .notifications
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .len();
            if count >= target {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "expected {target} subscription notifications within 5s"
            );
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }

    fn notification_count(ctx: &MockContext) -> usize {
        ctx.notifications
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .len()
    }

    /// End-to-end lifecycle of one per-profile subscription job on the
    /// unified JobScheduler: enabling schedules a job, the job fires the
    /// existing update logic (immediate first run, then again once the
    /// profile is re-armed as due), and disabling cancels the job so it
    /// never fires again.
    #[tokio::test]
    async fn test_subscription_job_enable_trigger_disable() {
        let _guard = TEST_LOCK.lock().await;
        let temp_dir = tempfile::Builder::new()
            .prefix("sub-test-job-")
            .tempdir()
            .unwrap();
        mihomo_platform::paths::clear_home_dir_override();
        mihomo_platform::paths::set_home_dir_override(temp_dir.path().to_path_buf());

        let mut server = mockito::Server::new_async().await;
        // Bound to `_mock` so it stays registered on the server for the
        // whole test (dropping a Mock removes it).
        let _mock = server
            .mock("GET", "/sub")
            .with_status(200)
            .with_body("port: 7890\nmode: rule")
            .create_async()
            .await;
        let subscription_url = format!("{}/sub", server.url());

        let manager = ConfigManager::new().unwrap();
        let profile_name = "job-lifecycle".to_string();
        let configs_dir = temp_dir.path().join("configs");
        let _ = std::fs::create_dir_all(&configs_dir);
        let profile_path = configs_dir.join(format!("{}.yaml", profile_name));
        std::fs::write(&profile_path, "port: 7890").unwrap();

        let mut profile = Profile::new(profile_name.clone(), profile_path, false);
        profile.subscription_url = Some(subscription_url.clone());
        profile.auto_update_enabled = true;
        profile.update_interval_hours = Some(1);
        // Due immediately so the job's first run performs a real update.
        profile.next_update = None;
        manager
            .update_profile_metadata(&profile_name, &profile)
            .await
            .unwrap();

        let ctx = MockContext {
            notifications: Arc::new(Mutex::new(vec![])),
        };

        // 启用：register the job. Production derives the interval from
        // update_interval_hours; a small interval keeps the test fast.
        schedule_profile_update_job(&ctx, &profile_name, Duration::from_millis(20));
        let job_name = subscription_job_name(&profile_name);
        assert!(subscription_jobs().is_active(&job_name));

        // 到点触发：first run is immediate and runs the existing update
        // logic end to end (fetch + save + metadata + notification).
        wait_for_notifications(&ctx, 1).await;
        assert_eq!(notification_count(&ctx), 1);
        let snapshot = subscription_jobs()
            .snapshot()
            .into_iter()
            .find(|snap| snap.name == job_name)
            .expect("job should stay registered");
        assert!(snapshot.active);
        assert!(snapshot.run_count >= 1);
        assert_eq!(snapshot.failure_count, 0);
        assert_eq!(snapshot.last_error, None);

        // A following tick is not due yet (next_update was pushed by the
        // successful update), so the run count stays flat until re-armed.
        tokio::time::sleep(Duration::from_millis(80)).await;
        let runs_after_first = subscription_jobs()
            .snapshot()
            .into_iter()
            .find(|snap| snap.name == job_name)
            .expect("job should stay registered")
            .run_count;
        assert_eq!(notification_count(&ctx), 1);
        assert!(runs_after_first >= 1);

        // Re-arm the profile as due; the next tick must trigger again.
        let mut metadata = manager.get_profile_metadata(&profile_name).await.unwrap();
        metadata.next_update = None;
        manager
            .update_profile_metadata(&profile_name, &metadata)
            .await
            .unwrap();
        wait_for_notifications(&ctx, 2).await;

        // 关闭：auto-update switched off in metadata cancels the job.
        sync_profile_job(&ctx, &profile_name, false, Some(&subscription_url), Some(1));
        assert!(!subscription_jobs().is_active(&job_name));
        assert!(subscription_jobs().snapshot().is_empty());

        // 不再触发：let several intervals pass; no more fetches and no more
        // notifications may land.
        tokio::time::sleep(Duration::from_millis(150)).await;
        let notifications_after_cancel = notification_count(&ctx);
        tokio::time::sleep(Duration::from_millis(120)).await;
        assert_eq!(
            notification_count(&ctx),
            notifications_after_cancel,
            "canceled job must not trigger again"
        );

        // Cleanup: keep the process-wide registry empty for other tests.
        cancel_profile_update_job(&profile_name);
        mihomo_platform::paths::clear_home_dir_override();
    }
}
