//! Runtime-neutral profile projections and profile-name validation.
//!
//! ConfigManager and subscription/file operations stay in
//! `infiltrator-core::profiles`; these values are the owned data exchanged
//! with inbound surfaces.

use chrono::{DateTime, Utc};
use serde::Serialize;

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct ProfileInfo {
    pub name: String,
    pub active: bool,
    pub path: String,
    pub controller_url: Option<String>,
    pub controller_changed: Option<bool>,
    pub subscription_url: Option<String>,
    pub auto_update_enabled: bool,
    pub update_interval_hours: Option<u32>,
    pub last_updated: Option<DateTime<Utc>>,
    pub next_update: Option<DateTime<Utc>>,
    pub traffic_upload: Option<u64>,
    pub traffic_download: Option<u64>,
    pub traffic_total: Option<u64>,
    pub expire_at: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct ProfileDetail {
    pub name: String,
    pub active: bool,
    pub path: String,
    pub content: String,
    pub subscription_url: Option<String>,
    pub auto_update_enabled: bool,
    pub update_interval_hours: Option<u32>,
    pub last_updated: Option<DateTime<Utc>>,
    pub next_update: Option<DateTime<Utc>>,
    pub traffic_upload: Option<u64>,
    pub traffic_download: Option<u64>,
    pub traffic_total: Option<u64>,
    pub expire_at: Option<i64>,
}

pub fn sanitize_profile_name(name: &str) -> anyhow::Result<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(anyhow::anyhow!("配置名称不能为空"));
    }
    if trimmed
        .chars()
        .any(|ch| matches!(ch, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'))
    {
        return Err(anyhow::anyhow!("配置名称不能包含特殊字符 / \\\\ : * ? \\\" < > |"));
    }
    Ok(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::sanitize_profile_name;

    #[test]
    fn valid_names_are_trimmed() {
        assert_eq!(sanitize_profile_name("  valid_name  ").unwrap(), "valid_name");
    }

    #[test]
    fn path_like_names_are_rejected() {
        assert!(sanitize_profile_name("invalid/name").is_err());
        assert!(sanitize_profile_name("invalid\\name").is_err());
    }
}
