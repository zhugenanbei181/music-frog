use crate::yaml;
use chrono::{DateTime, Utc};
use mihomo_api::{MihomoError, Result};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Profile {
    pub name: String,
    pub path: PathBuf,
    pub active: bool,
    pub subscription_url: Option<String>,
    pub auto_update_enabled: bool,
    pub update_interval_hours: Option<u32>,
    pub last_updated: Option<DateTime<Utc>>,
    pub next_update: Option<DateTime<Utc>>,
}

impl Profile {
    pub fn new(name: String, path: PathBuf, active: bool) -> Self {
        Self {
            name,
            path,
            active,
            subscription_url: None,
            auto_update_enabled: false,
            update_interval_hours: None,
            last_updated: None,
            next_update: None,
        }
    }

    pub async fn validate(&self) -> Result<()> {
        if !self.path.exists() {
            return Err(MihomoError::Config(format!(
                "Profile file does not exist: {}",
                self.path.display()
            )));
        }

        let content = tokio::fs::read_to_string(&self.path).await?;
        yaml::validate(&content)?;

        Ok(())
    }

    pub async fn backup(&self) -> Result<PathBuf> {
        let backup_path = self.path.with_extension("yaml.bak");
        tokio::fs::copy(&self.path, &backup_path).await?;
        Ok(backup_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;

    #[tokio::test]
    async fn test_profile_new() {
        let profile = Profile::new(
            "test-profile".to_string(),
            PathBuf::from("/tmp/test.yaml"),
            true,
        );

        assert_eq!(profile.name, "test-profile");
        assert_eq!(profile.path, PathBuf::from("/tmp/test.yaml"));
        assert!(profile.active);
        assert_eq!(profile.subscription_url, None);
        assert!(!profile.auto_update_enabled);
        assert_eq!(profile.update_interval_hours, None);
        assert_eq!(profile.last_updated, None);
        assert_eq!(profile.next_update, None);
    }

    #[tokio::test]
    async fn test_profile_validate_success() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "port: 7890").unwrap();
        writeln!(temp_file, "socks-port: 7891").unwrap();
        writeln!(temp_file, "mode: rule").unwrap();

        let profile = Profile::new(
            "test".to_string(),
            temp_file.path().to_path_buf(),
            false,
        );

        let result = profile.validate().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_profile_validate_nonexistent_file() {
        let profile = Profile::new(
            "test".to_string(),
            PathBuf::from("/nonexistent/path.yaml"),
            false,
        );

        let result = profile.validate().await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("does not exist"));
    }

    #[tokio::test]
    async fn test_profile_validate_invalid_yaml() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "invalid: yaml: content: [").unwrap();

        let profile = Profile::new(
            "test".to_string(),
            temp_file.path().to_path_buf(),
            false,
        );

        let result = profile.validate().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_profile_backup() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "port: 7890").unwrap();

        let profile = Profile::new(
            "test".to_string(),
            temp_file.path().to_path_buf(),
            false,
        );

        let result = profile.backup().await;
        assert!(result.is_ok());

        let backup_path = result.unwrap();
        assert!(backup_path.exists());
        assert!(backup_path.to_str().unwrap().ends_with(".yaml.bak"));

        // Clean up backup
        std::fs::remove_file(backup_path).unwrap();
    }

    #[test]
    fn test_profile_clone() {
        let profile = Profile::new(
            "test".to_string(),
            PathBuf::from("/tmp/test.yaml"),
            true,
        );

        let cloned = profile.clone();
        assert_eq!(cloned.name, profile.name);
        assert_eq!(cloned.path, profile.path);
        assert_eq!(cloned.active, profile.active);
    }

    #[test]
    fn test_profile_debug() {
        let profile = Profile::new(
            "test".to_string(),
            PathBuf::from("/tmp/test.yaml"),
            false,
        );

        let debug_str = format!("{:?}", profile);
        assert!(debug_str.contains("test"));
    }
}
