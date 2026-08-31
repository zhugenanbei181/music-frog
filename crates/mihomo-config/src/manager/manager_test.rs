//! Tests for the [`crate::manager`] module, mounted via `#[cfg(test)]` from
//! the module root (kept out of the business-code line budget by convention).

#[cfg(test)]
mod tests {
    use crate::manager::ConfigManager;
    use crate::profile::Profile;
    use mihomo_platform::traits::CredentialStore;
    use std::path::PathBuf;
    use tempfile::TempDir;
    use tokio::fs;

    async fn setup_test_manager(temp_dir: &TempDir) -> ConfigManager {
        let home = temp_dir.path().to_path_buf();
        ConfigManager::with_home(home).unwrap()
    }

    #[tokio::test]
    async fn test_manager_new() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir).await;

        assert_eq!(manager.config_dir, temp_dir.path().join("configs"));
        assert_eq!(manager.settings_file, temp_dir.path().join("config.toml"));
    }

    #[tokio::test]
    async fn test_save_profile_success() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir).await;

        let profile_content = "port: 7890\nsocks-port: 7891\nmode: rule";
        let result = manager.save("test-profile", profile_content).await;

        assert!(result.is_ok());
        let profile_path = manager.config_dir.join("test-profile.yaml");
        assert!(profile_path.exists());

        let content = fs::read_to_string(&profile_path).await.unwrap();
        assert!(content.contains("port: 7890"));
    }

    #[tokio::test]
    async fn test_save_keeps_one_shot_backup_until_apply_clears_it() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir).await;
        manager.save("current", "port: 7890\n").await.unwrap();
        manager.save("current", "port: 7891\n").await.unwrap();

        let path = manager.config_dir.join("current.yaml");
        let backup = manager.config_dir.join("current.yaml.bak");
        assert_eq!(fs::read_to_string(&path).await.unwrap(), "port: 7891\n");
        assert_eq!(fs::read_to_string(&backup).await.unwrap(), "port: 7890\n");

        assert!(manager.restore_backup("current").await.unwrap());
        assert_eq!(fs::read_to_string(&path).await.unwrap(), "port: 7890\n");
        manager.clear_backup("current").await.unwrap();
        assert!(!backup.exists());
    }

    #[tokio::test]
    async fn test_save_invalid_yaml() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir).await;

        let profile_content = "invalid: yaml: content: [";
        let result = manager.save("test-profile", profile_content).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_load_profile_success() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir).await;

        let profile_content = "port: 7890\nsocks-port: 7891\nmode: rule";
        manager.save("test-profile", profile_content).await.unwrap();

        let result = manager.load("test-profile").await;
        assert!(result.is_ok());
        assert!(result.unwrap().contains("port: 7890"));
    }

    #[tokio::test]
    async fn test_load_nonexistent_profile() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir).await;

        let result = manager.load("nonexistent").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[tokio::test]
    async fn test_list_profiles_empty() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir).await;

        let result = manager.list_profiles().await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_list_profiles_with_files() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir).await;

        manager.save("profile1", "port: 7890").await.unwrap();
        manager.save("profile2", "port: 7891").await.unwrap();

        let result = manager.list_profiles().await;
        assert!(result.is_ok());
        let profiles = result.unwrap();
        assert_eq!(profiles.len(), 2);
        assert!(profiles.iter().any(|p| p.name == "profile1"));
        assert!(profiles.iter().any(|p| p.name == "profile2"));
    }

    #[tokio::test]
    async fn test_list_profiles_sorted() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir).await;

        manager.save("z-profile", "port: 7890").await.unwrap();
        manager.save("a-profile", "port: 7891").await.unwrap();
        manager.save("m-profile", "port: 7892").await.unwrap();

        let result = manager.list_profiles().await;
        assert!(result.is_ok());
        let profiles = result.unwrap();
        assert_eq!(profiles[0].name, "a-profile");
        assert_eq!(profiles[1].name, "m-profile");
        assert_eq!(profiles[2].name, "z-profile");
    }

    #[tokio::test]
    async fn test_delete_profile_success() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir).await;

        manager.save("test-profile", "port: 7890").await.unwrap();
        assert!(manager.config_dir.join("test-profile.yaml").exists());

        let result = manager.delete_profile("test-profile").await;
        assert!(result.is_ok());
        assert!(!manager.config_dir.join("test-profile.yaml").exists());
    }

    #[tokio::test]
    async fn test_delete_nonexistent_profile() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir).await;

        let result = manager.delete_profile("nonexistent").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[tokio::test]
    async fn test_delete_active_profile() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir).await;

        manager.save("active-profile", "port: 7890").await.unwrap();
        manager.set_current("active-profile").await.unwrap();

        let result = manager.delete_profile("active-profile").await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Cannot delete the active profile")
        );
    }

    #[tokio::test]
    async fn test_set_current_profile() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir).await;

        manager.save("test-profile", "port: 7890").await.unwrap();

        let result = manager.set_current("test-profile").await;
        assert!(result.is_ok());

        let current = manager.get_current().await.unwrap();
        assert_eq!(current, "test-profile");
    }

    #[tokio::test]
    async fn test_set_nonexistent_current_profile() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir).await;

        let result = manager.set_current("nonexistent").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[tokio::test]
    async fn test_get_current_default() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir).await;

        let current = manager.get_current().await.unwrap();
        assert_eq!(current, "default");
    }

    #[tokio::test]
    async fn test_get_current_path() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir).await;

        manager.save("test-profile", "port: 7890").await.unwrap();
        manager.set_current("test-profile").await.unwrap();

        let path = manager.get_current_path().await.unwrap();
        assert_eq!(path, manager.config_dir.join("test-profile.yaml"));
    }

    #[tokio::test]
    async fn test_ensure_default_config_creates() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir).await;

        let result = manager.ensure_default_config().await;
        assert!(result.is_ok());

        let current = manager.get_current().await.unwrap();
        assert!(
            manager
                .config_dir
                .join(format!("{}.yaml", current))
                .exists()
        );
    }

    #[tokio::test]
    async fn test_ensure_default_config_exists() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir).await;

        manager.save("default", "port: 7890").await.unwrap();

        let result = manager.ensure_default_config().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_external_controller() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir).await;

        let config = r#"
port: 7890
external-controller: 127.0.0.1:9090
"#;
        manager.save("default", config).await.unwrap();

        let result = manager.get_external_controller().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "http://127.0.0.1:9090");
    }

    #[tokio::test]
    async fn test_get_external_controller_default() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir).await;

        let config = r#"
port: 7890
"#;
        manager.save("default", config).await.unwrap();

        let result = manager.get_external_controller().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "http://127.0.0.1:9090");
    }

    #[tokio::test]
    async fn test_get_external_controller_with_colon() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir).await;

        let config = r#"
port: 7890
external-controller: ":9090"
"#;
        manager.save("default", config).await.unwrap();

        let result = manager.get_external_controller().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "http://127.0.0.1:9090");
    }

    #[tokio::test]
    async fn test_get_external_controller_with_http() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir).await;

        let config = r#"
port: 7890
external-controller: http://127.0.0.1:9090
"#;
        manager.save("default", config).await.unwrap();

        let result = manager.get_external_controller().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "http://127.0.0.1:9090");
    }

    #[tokio::test]
    async fn test_update_profile_metadata() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir).await;

        let metadata = Profile {
            name: "test".to_string(),
            path: PathBuf::new(),
            active: false,
            subscription_url: Some("http://example.com".to_string()),
            auto_update_enabled: true,
            update_interval_hours: Some(24),
            last_updated: None,
            next_update: None,
            traffic_upload: None,
            traffic_download: None,
            traffic_total: None,
            expire_at: None,
        };

        let result = manager
            .update_profile_metadata("test-profile", &metadata)
            .await;
        assert!(result.is_ok());

        let retrieved = manager.get_profile_metadata("test-profile").await.unwrap();
        assert!(retrieved.auto_update_enabled);
        assert_eq!(retrieved.update_interval_hours, Some(24));
    }

    #[tokio::test]
    async fn test_get_profile_metadata_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir).await;

        let result = manager.get_profile_metadata("nonexistent").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().name, "nonexistent");
    }

    #[tokio::test]
    async fn test_secured_storage_integration() {
        use async_trait::async_trait;
        use std::collections::HashMap;
        use std::sync::{Arc, Mutex};

        #[derive(Default, Clone)]
        struct MockStore {
            data: Arc<Mutex<HashMap<String, String>>>,
        }

        #[async_trait]
        impl CredentialStore for MockStore {
            async fn get(
                &self,
                _svc: &str,
                key: &str,
            ) -> mihomo_api::error::Result<Option<String>> {
                Ok(self.data.lock().unwrap().get(key).cloned())
            }
            async fn set(&self, _svc: &str, key: &str, val: &str) -> mihomo_api::error::Result<()> {
                self.data
                    .lock()
                    .unwrap()
                    .insert(key.to_string(), val.to_string());
                Ok(())
            }
            async fn delete(&self, _svc: &str, key: &str) -> mihomo_api::error::Result<()> {
                self.data.lock().unwrap().remove(key);
                Ok(())
            }
        }

        let temp_dir = TempDir::new().unwrap();
        let store = MockStore::default();
        let manager =
            ConfigManager::with_home_and_store(temp_dir.path().to_path_buf(), store.clone())
                .unwrap();

        let mut metadata = Profile::new("test".to_string(), PathBuf::new(), false);
        metadata.subscription_url = Some("https://secret.url/sub".to_string());

        // 1. Save metadata
        manager
            .update_profile_metadata("test", &metadata)
            .await
            .unwrap();

        // 2. Verify store has the secret
        let key = "subscription:test".to_string();
        assert_eq!(
            store.data.lock().unwrap().get(&key).unwrap(),
            "https://secret.url/sub"
        );

        // 3. Load metadata and verify url is recovered
        let loaded = manager.get_profile_metadata("test").await.unwrap();
        assert_eq!(
            loaded.subscription_url,
            Some("https://secret.url/sub".to_string())
        );
    }

    #[tokio::test]
    async fn test_profile_name_validation_blocks_traversal() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir).await;

        for name in [
            "../escape",
            "sub/../../escape",
            "/absolute/path",
            "windows\\path",
            "drive:c:\\x",
            "..",
            ".",
            " padded ",
            "",
            "a:b",
        ] {
            assert!(
                manager.save(name, "port: 1\n").await.is_err(),
                "save should reject: {name:?}"
            );
            assert!(
                manager.load(name).await.is_err(),
                "load should reject: {name:?}"
            );
            assert!(
                manager.delete_profile(name).await.is_err(),
                "delete should reject: {name:?}"
            );
        }

        // 合法名字不受影响（空格/中文/下划线/连字符）
        assert!(manager.save("我的 配置-1", "port: 1\n").await.is_ok());
        assert_eq!(manager.load("我的 配置-1").await.unwrap(), "port: 1\n");
    }
}
