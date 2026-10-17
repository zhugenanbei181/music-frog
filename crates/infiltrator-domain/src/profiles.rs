//! Runtime-neutral profile projections and profile-name sanitization.
//!
//! ConfigManager and subscription/file operations stay in outbound adapters;
//! these values are the owned data exchanged with application and inbound
//! surfaces.

use chrono::{DateTime, Utc};
use lurker_core_contract::profile_name::validate_profile_name;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProfileMetadata {
    pub subscription_url: Option<String>,
    pub auto_update_enabled: bool,
    pub update_interval_hours: Option<u32>,
    pub last_updated: Option<DateTime<Utc>>,
    pub next_update: Option<DateTime<Utc>>,
    pub traffic_upload: Option<u64>,
    pub traffic_download: Option<u64>,
    pub traffic_total: Option<u64>,
    pub expire_at: Option<i64>,
    pub user_agent: Option<String>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub cron_expression: Option<String>,
    pub insecure_skip_verify: bool,
    pub auto_reload_core: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
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
    pub user_agent: Option<String>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub cron_expression: Option<String>,
    pub insecure_skip_verify: bool,
    pub auto_reload_core: bool,
    /// Whether a transient pre-save `.bak` copy currently exists for this
    /// profile (DUAL-07-13). Surfaces use it to offer a safe restore.
    pub has_backup: bool,
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
    validate_profile_name(trimmed)?;
    Ok(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::sanitize_profile_name;
    use lurker_core_contract::profile_name::validate_profile_name;

    #[test]
    fn valid_names_are_trimmed() {
        assert_eq!(
            sanitize_profile_name("  valid_name  ").unwrap(),
            "valid_name"
        );
    }

    #[test]
    fn path_like_names_are_rejected() {
        assert!(sanitize_profile_name("invalid/name").is_err());
        assert!(sanitize_profile_name("invalid\\name").is_err());
    }

    #[test]
    fn names_are_portable_across_engine_file_stores() {
        for invalid in ["release..backup", "trailing.", "trailing ", "a?b"] {
            assert!(validate_profile_name(invalid).is_err(), "{invalid:?}");
        }
        assert!(validate_profile_name("regional.v1").is_ok());
        assert_eq!(
            sanitize_profile_name("  regional.v1  ").unwrap(),
            "regional.v1"
        );
    }
}
