#[cfg(test)]
mod tests {
    use crate::admin_api::AdminApiContext;
    use infiltrator_http::HttpClient;
    use mihomo_config::{ConfigManager, Profile};
    use chrono::{Utc, Duration as ChronoDuration};
    use std::sync::{Arc, Mutex};
    use crate::scheduler::subscription::{SubscriptionUpdateSummary, update_all_subscriptions, schedule_next_attempt};
    use infiltrator_core::subscription::mask_subscription_url;
    use infiltrator_core::AppSettings;
    use std::sync::LazyLock;

    static TEST_MUTEX: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    #[derive(Clone)]
    struct MockContext {
        notifications: Arc<Mutex<Vec<(String, bool, Option<String>)>>>,
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
                .unwrap()
                .push((profile, success, message));
        }

        async fn rebuild_runtime(&self) -> anyhow::Result<()> {
            Ok(())
        }

        async fn set_use_bundled_core(&self, _enabled: bool) {}
        async fn refresh_core_version_info(&self) {}
        async fn editor_path(&self) -> Option<String> { None }
        async fn set_editor_path(&self, _path: Option<String>) {}
        async fn pick_editor_path(&self) -> Option<String> { None }
        async fn open_profile_in_editor(&self, _profile_name: &str) -> anyhow::Result<()> { Ok(()) }
        async fn get_app_settings(&self) -> AppSettings { AppSettings::default() }
        async fn save_app_settings(&self, _settings: AppSettings) -> anyhow::Result<()> { Ok(()) }
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
        let _guard = TEST_MUTEX.lock().unwrap();
        let temp_dir = tempfile::Builder::new().prefix("sub-test-none-").tempdir().unwrap();
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
        let _guard = TEST_MUTEX.lock().unwrap();
        let temp_dir = tempfile::Builder::new().prefix("sub-test-parallel-").tempdir().unwrap();
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
            
            let mut profile = Profile::new(
                profile_name.clone(),
                profile_path,
                false,
            );
            profile.subscription_url = Some(format!("http://example.com/subscription/{}", i));
            profile.auto_update_enabled = true;
            profile.update_interval_hours = Some(24);
            
            manager.update_profile_metadata(&profile_name, &profile).await.unwrap();
        }

        let profiles = manager.list_profiles().await.unwrap();
        assert!(profiles.len() >= 10, "Manager should see at least 10 profiles, but saw {}", profiles.len());

        let result = update_all_subscriptions(&ctx, &client, &raw_client).await;

        assert!(result.is_ok());
        let summary = result.unwrap();
        assert!(summary.total >= 10, "Summary total should be >= 10, but was {}", summary.total);

        mihomo_platform::clear_home_dir_override();
    }

    #[tokio::test]
    async fn test_mask_subscription_url() {
        assert_eq!(
            mask_subscription_url("https://example.com/link/abcdefg123456?mu=0"),
            "https://example.com/link/***?mu=0"
        );

        assert_eq!(
            mask_subscription_url("https://example.com/link/abcdefg123456"),
            "https://example.com/link/***"
        );

        assert_eq!(
            mask_subscription_url("https://google.com"),
            "https://google.com"
        );
    }

    #[tokio::test]
    async fn test_schedule_next_attempt() {
        let _guard = TEST_MUTEX.lock().unwrap();
        let temp_dir = tempfile::Builder::new().prefix("sub-test-schedule-").tempdir().unwrap();
        mihomo_platform::clear_home_dir_override();
        mihomo_platform::set_home_dir_override(temp_dir.path().to_path_buf());
        
        let manager = ConfigManager::new().unwrap();

        let profile_name = "test-schedule".to_string();
        let configs_dir = temp_dir.path().join("configs");
        let _ = std::fs::create_dir_all(&configs_dir);
        let profile_path = configs_dir.join(format!("{}.yaml", profile_name));
        let _ = std::fs::write(&profile_path, "port: 7890");

        let profile = Profile::new(
            profile_name.clone(),
            profile_path,
            false,
        );

        let now = Utc::now();
        let interval_hours = 24u32;

        schedule_next_attempt(&manager, &profile, interval_hours, now).await.unwrap();

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
