#[cfg(test)]
mod tests {
    use super::super::run_sync_tick;
    use crate::TEST_LOCK;
    use crate::admin_api::AdminApiContext;
    use anyhow::anyhow;
    use infiltrator_core::AppSettings;
    use infiltrator_core::settings::WebDavConfig;
    use mihomo_api::MihomoClient;

    #[derive(Clone)]
    struct MockContext;

    #[async_trait::async_trait]
    impl AdminApiContext for MockContext {
        async fn notify_subscription_update(
            &self,
            _profile: String,
            _success: bool,
            _message: Option<String>,
        ) {
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
            Err(anyhow!("runtime client is not available in sync tests"))
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
    async fn test_run_sync_tick_disabled() {
        let ctx = MockContext;
        let config = WebDavConfig::default(); // default enabled is false

        let result = run_sync_tick(&ctx, &config).await;
        assert!(result.is_ok());
        let summary = result.unwrap();
        assert_eq!(summary.total_actions, 0);
    }

    #[tokio::test]
    async fn test_run_sync_tick_empty_url() {
        let ctx = MockContext;
        let config = WebDavConfig {
            enabled: true,
            url: "".to_string(),
            ..WebDavConfig::default()
        };

        let result = run_sync_tick(&ctx, &config).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "WebDAV URL is empty");
    }

    #[tokio::test]
    async fn test_run_sync_tick_invalid_url() {
        let _guard = TEST_LOCK.lock().await;
        let temp_dir = tempfile::tempdir().unwrap();
        mihomo_platform::set_home_dir_override(temp_dir.path().to_path_buf());

        let ctx = MockContext;
        let config = WebDavConfig {
            enabled: true,
            url: "not-a-url".to_string(),
            ..WebDavConfig::default()
        };

        let result = run_sync_tick(&ctx, &config).await;
        // Should fail during client creation or plan building
        assert!(result.is_err());

        mihomo_platform::clear_home_dir_override();
    }
}
