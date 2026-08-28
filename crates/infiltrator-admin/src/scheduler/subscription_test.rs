#[cfg(test)]
mod tests {
    use crate::TEST_LOCK;
    use crate::admin_api::AdminApiContext;
    use crate::scheduler::subscription::{
        SubscriptionUpdateSummary, schedule_next_attempt, update_all_subscriptions,
    };
    use anyhow::anyhow;
    use chrono::{Duration as ChronoDuration, Utc};
    use infiltrator_core::AppSettings;
    use infiltrator_core::subscription::mask_subscription_url;
    use infiltrator_http::HttpClient;
    use mihomo_api::MihomoClient;
    use mihomo_config::{ConfigManager, Profile};
    use std::sync::{Arc, Mutex};

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
        mihomo_platform::clear_home_dir_override();
        mihomo_platform::set_home_dir_override(temp_dir.path().to_path_buf());

        let ctx = MockContext {
            notifications: Arc::new(Mutex::new(vec![])),
        };
        let client = HttpClient::new();
        let raw_client = HttpClient::new();

        let result = update_all_subscriptions(&ctx, &client, &raw_client).await;

        assert!(result.is_ok());
        let summary = result.unwrap();
        assert!(summary.total <= 1);

        mihomo_platform::clear_home_dir_override();
    }

    #[tokio::test]
    async fn test_update_all_subscriptions_parallel_concurrency() {
        let _guard = TEST_LOCK.lock().await;
        let temp_dir = tempfile::Builder::new()
            .prefix("sub-test-parallel-")
            .tempdir()
            .unwrap();
        mihomo_platform::clear_home_dir_override();
        mihomo_platform::set_home_dir_override(temp_dir.path().to_path_buf());

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

        mihomo_platform::clear_home_dir_override();
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
        mihomo_platform::clear_home_dir_override();
        mihomo_platform::set_home_dir_override(temp_dir.path().to_path_buf());

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

        mihomo_platform::clear_home_dir_override();
    }
}
