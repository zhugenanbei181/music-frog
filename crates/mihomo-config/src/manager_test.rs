#[cfg(test)]
mod tests {
    use super::*;
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
    async fn test_save_invalid_yaml() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir).await;

        let profile_content = "invalid: yaml: content: [";
        let result = manager.save("test-profile", profile_content).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid YAML"));
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
        assert!(result.unwrap_err().to_string().contains("Cannot delete the active profile"));
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
        assert!(manager.config_dir.join(format!("{}.yaml", current)).exists());
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
        };

        let result = manager.update_profile_metadata("test-profile", &metadata).await;
        assert!(result.is_ok());

        let retrieved = manager.get_profile_metadata("test-profile").await.unwrap();
        assert_eq!(retrieved.auto_update_enabled, true);
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
}
