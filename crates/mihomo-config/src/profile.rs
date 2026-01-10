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
