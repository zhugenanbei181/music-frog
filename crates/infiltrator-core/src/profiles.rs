use anyhow::anyhow;
use chrono::{DateTime, Utc};
use mihomo_config::{port::find_available_port, ConfigManager, Profile as MihomoProfile};
use mihomo_platform::get_home_dir;
use serde::Serialize;
use tokio::fs;
use crate::{config as core_config, subscription as core_subscription};
use infiltrator_http::{build_http_client, build_raw_http_client};

#[derive(Debug, Clone, Serialize)]
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
}

pub fn profile_to_info(profile: MihomoProfile) -> ProfileInfo {
    ProfileInfo {
        name: profile.name,
        active: profile.active,
        path: profile.path.to_string_lossy().to_string(),
        controller_url: None,
        controller_changed: None,
        subscription_url: profile.subscription_url,
        auto_update_enabled: profile.auto_update_enabled,
        update_interval_hours: profile.update_interval_hours,
        last_updated: profile.last_updated,
        next_update: profile.next_update,
    }
}

pub async fn load_profile_info(name: &str) -> anyhow::Result<ProfileInfo> {
    let cm = ConfigManager::new()?;
    let profiles = cm.list_profiles().await?;
    profiles
        .into_iter()
        .find(|profile| profile.name == name)
        .map(profile_to_info)
        .ok_or_else(|| anyhow!(format!("未找到名称为 {name} 的配置文件")))
}

pub async fn list_profile_infos() -> anyhow::Result<Vec<ProfileInfo>> {
    let cm = ConfigManager::new()?;
    let profiles = cm.list_profiles().await?;
    Ok(profiles.into_iter().map(profile_to_info).collect())
}

pub async fn create_profile_from_url(name: &str, url: &str) -> anyhow::Result<ProfileInfo> {
    let profile_name = sanitize_profile_name(name)?;
    let source_url = url.trim();
    if source_url.is_empty() {
        return Err(anyhow!("订阅链接不能为空"));
    }

    let client = build_http_client();
    let raw_client = build_raw_http_client(&client);
    let content =
        core_subscription::fetch_subscription_text(&client, &raw_client, source_url).await?;
    let content = core_subscription::strip_utf8_bom(&content);
    if core_config::validate_yaml(&content).is_err() {
        return Err(anyhow!("订阅内容不是有效的 YAML"));
    }

    let manager = ConfigManager::new()?;
    manager.save(&profile_name, &content).await?;

    let now = Utc::now();
    let mut metadata = manager.get_profile_metadata(&profile_name).await?;
    metadata.subscription_url = Some(source_url.to_string());
    metadata.last_updated = Some(now);
    metadata.next_update = if metadata.auto_update_enabled {
        metadata
            .update_interval_hours
            .map(|hours| now + chrono::Duration::hours(hours as i64))
    } else {
        None
    };
    manager
        .update_profile_metadata(&profile_name, &metadata)
        .await?;

    load_profile_info(&profile_name).await
}

pub async fn select_profile(name: &str) -> anyhow::Result<ProfileInfo> {
    let profile_name = sanitize_profile_name(name)?;
    let manager = ConfigManager::new()?;
    manager.set_current(&profile_name).await?;
    load_profile_info(&profile_name).await
}

pub async fn update_profile(name: &str) -> anyhow::Result<ProfileInfo> {
    let profile_name = sanitize_profile_name(name)?;
    let manager = ConfigManager::new()?;
    let mut metadata = manager.get_profile_metadata(&profile_name).await?;
    let url = metadata
        .subscription_url
        .as_deref()
        .ok_or_else(|| anyhow!("未找到订阅链接"))?;

    let client = build_http_client();
    let raw_client = build_raw_http_client(&client);
    let content = core_subscription::fetch_subscription_text(&client, &raw_client, url).await?;
    let content = core_subscription::strip_utf8_bom(&content);
    if core_config::validate_yaml(&content).is_err() {
        return Err(anyhow!("订阅内容不是有效的 YAML"));
    }
    manager.save(&profile_name, &content).await?;

    let now = Utc::now();
    metadata.last_updated = Some(now);
    metadata.next_update = if metadata.auto_update_enabled {
        metadata
            .update_interval_hours
            .map(|hours| now + chrono::Duration::hours(hours as i64))
    } else {
        None
    };
    manager
        .update_profile_metadata(&profile_name, &metadata)
        .await?;

    load_profile_info(&profile_name).await
}

pub async fn load_profile_detail(name: &str) -> anyhow::Result<ProfileDetail> {
    let profile = load_profile_info(name).await?;
    let manager = ConfigManager::new()?;
    let content = manager.load(&profile.name).await?;
    Ok(ProfileDetail {
        name: profile.name,
        active: profile.active,
        path: profile.path,
        content,
        subscription_url: profile.subscription_url,
        auto_update_enabled: profile.auto_update_enabled,
        update_interval_hours: profile.update_interval_hours,
        last_updated: profile.last_updated,
        next_update: profile.next_update,
    })
}

pub fn sanitize_profile_name(name: &str) -> anyhow::Result<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("配置名称不能为空"));
    }
    if trimmed
        .chars()
        .any(|ch| matches!(ch, '/' | '\\' | ':' | '*' | '?' | '\"' | '<' | '>' | '|'))
    {
        return Err(anyhow!(
            "配置名称不能包含特殊字符 / \\\\ : * ? \\\" < > |"
        ));
    }
    Ok(trimmed.to_string())
}

pub async fn reset_profiles_to_default() -> anyhow::Result<ProfileInfo> {
    let home = get_home_dir()?;
    let config_dir = home.join("configs");
    if config_dir.exists() {
        fs::remove_dir_all(&config_dir).await?;
    }

    let manager = ConfigManager::with_home(home)?;
    let default_config = build_default_config()?;
    manager.save("default", &default_config).await?;
    manager.set_current("default").await?;
    load_profile_info("default").await
}

fn build_default_config() -> anyhow::Result<String> {
    let port = find_available_port(9090).ok_or_else(|| {
        anyhow!("无法找到可用的控制接口端口（9090-9190）")
    })?;
    Ok(format!(
        r#"# mihomo configuration
port: 7890
socks-port: 7891
allow-lan: false
mode: rule
log-level: info
external-controller: 127.0.0.1:{}
"#,
        port
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_profile_name_valid() {
        assert_eq!(sanitize_profile_name("  valid_name  ").unwrap(), "valid_name");
        assert_eq!(sanitize_profile_name("config-1").unwrap(), "config-1");
    }

    #[test]
    fn test_sanitize_profile_name_invalid() {
        assert!(sanitize_profile_name("").is_err());
        assert!(sanitize_profile_name("   ").is_err());
        assert!(sanitize_profile_name("invalid/name").is_err());
        assert!(sanitize_profile_name("invalid\\name").is_err());
        assert!(sanitize_profile_name("invalid:name").is_err());
    }
}
