use anyhow::anyhow;
use chrono::Utc;
use infiltrator_http::{build_http_client, build_raw_http_client};
use mihomo_config::port::find_available_port;

use crate::settings_io::app_config_manager;
use infiltrator_domain::profiles::{ProfileDetail, ProfileInfo, sanitize_profile_name};
use infiltrator_domain::subscription::{CheckedSubscriptionUrl, strip_utf8_bom};
use tokio::fs;

pub fn profile_to_info(profile: mihomo_config::profile::Profile) -> ProfileInfo {
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
        traffic_upload: profile.traffic_upload,
        traffic_download: profile.traffic_download,
        traffic_total: profile.traffic_total,
        expire_at: profile.expire_at,
    }
}

pub async fn load_profile_info(name: &str) -> anyhow::Result<ProfileInfo> {
    let cm = app_config_manager().await?;
    let profiles = cm.list_profiles().await?;
    profiles
        .into_iter()
        .find(|profile| profile.name == name)
        .map(profile_to_info)
        .ok_or_else(|| anyhow!(format!("未找到名称为 {name} 的配置文件")))
}

pub async fn list_profile_infos() -> anyhow::Result<Vec<ProfileInfo>> {
    let cm = app_config_manager().await?;
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
    let checked_url = CheckedSubscriptionUrl::parse(source_url)?;
    let content =
        crate::subscription_io::fetch_subscription_text(&client, &raw_client, &checked_url).await?;
    let content = strip_utf8_bom(&content);
    let (content, _report) =
        crate::profile_options_io::apply_saved_options_for(&profile_name, content).await?;
    if infiltrator_domain::config::validate_yaml(&content).is_err() {
        return Err(anyhow!("订阅内容不是有效的 YAML"));
    }

    let manager = app_config_manager().await?;
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
    let manager = app_config_manager().await?;
    manager.set_current(&profile_name).await?;
    load_profile_info(&profile_name).await
}

pub async fn update_profile(name: &str) -> anyhow::Result<ProfileInfo> {
    let profile_name = sanitize_profile_name(name)?;
    let manager = app_config_manager().await?;
    let mut metadata = manager.get_profile_metadata(&profile_name).await?;
    let url = metadata
        .subscription_url
        .as_deref()
        .ok_or_else(|| anyhow!("未找到订阅链接"))?;

    let client = build_http_client();
    let raw_client = build_raw_http_client(&client);
    let checked_url = CheckedSubscriptionUrl::parse(url)?;
    let (content, userinfo) =
        crate::subscription_io::fetch_subscription_with_info(&client, &raw_client, &checked_url)
            .await?;
    let content = strip_utf8_bom(&content);
    let (content, _report) =
        crate::profile_options_io::apply_saved_options_for(&profile_name, content).await?;
    if infiltrator_domain::config::validate_yaml(&content).is_err() {
        return Err(anyhow!("订阅内容不是有效的 YAML"));
    }
    manager.save(&profile_name, &content).await?;

    let now = Utc::now();
    if let Some(info) = userinfo {
        metadata.traffic_upload = info.upload;
        metadata.traffic_download = info.download;
        metadata.traffic_total = info.total;
        metadata.expire_at = info.expire;
    }
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
    let manager = app_config_manager().await?;
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
        traffic_upload: profile.traffic_upload,
        traffic_download: profile.traffic_download,
        traffic_total: profile.traffic_total,
        expire_at: profile.expire_at,
    })
}

pub async fn reset_profiles_to_default() -> anyhow::Result<ProfileInfo> {
    let manager = app_config_manager().await?;
    // 清空并重建的是解析后的 configs 目录（可能已被云同步重定向），
    // 而不是固定的 `<home>/configs`。
    let config_dir = manager.config_dir().to_path_buf();
    if config_dir.exists() {
        fs::remove_dir_all(&config_dir).await?;
    }

    let default_config = build_default_config()?;
    manager.save("default", &default_config).await?;
    manager.set_current("default").await?;
    load_profile_info("default").await
}

fn build_default_config() -> anyhow::Result<String> {
    let port = find_available_port(9090)
        .ok_or_else(|| anyhow!("无法找到可用的控制接口端口（9090-9190）"))?;
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
        assert_eq!(
            sanitize_profile_name("  valid_name  ").unwrap(),
            "valid_name"
        );
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

    /// 门面（create/list/detail/select）必须全部落在 settings `configs_dir`
    /// 重定向后的目录；字段清空后回落默认目录，与旧行为一致。
    #[tokio::test]
    async fn test_profile_facades_follow_configs_dir_redirect() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().to_path_buf();
        let cloud = home.join("cloud-sync").join("profiles");
        std::fs::create_dir_all(&cloud).unwrap();
        let guard = crate::settings_io::test_support::RedirectGuard::acquire(home.clone()).await;
        guard
            .set_configs_dir(&home, Some(cloud.to_str().unwrap()))
            .await;

        // create：默认配置建到重定向目录。
        let created = reset_profiles_to_default().await.unwrap();
        assert!(std::path::Path::new(&created.path).starts_with(&cloud));
        assert!(!home.join("configs").exists());

        // list / detail / select 全部从重定向目录读取。
        let infos = list_profile_infos().await.unwrap();
        assert_eq!(infos.len(), 1);
        assert!(std::path::Path::new(&infos[0].path).starts_with(&cloud));
        let detail = load_profile_detail("default").await.unwrap();
        assert!(detail.content.contains("external-controller"));
        let selected = select_profile("default").await.unwrap();
        assert!(std::path::Path::new(&selected.path).starts_with(&cloud));

        // 字段清空：回落 `<home>/configs`，云端 profile 不可见。
        guard.set_configs_dir(&home, None).await;
        assert!(list_profile_infos().await.unwrap().is_empty());
    }
}
