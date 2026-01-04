use anyhow::anyhow;
use mihomo_rs::config::{ConfigManager, Profile as MihomoProfile};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ProfileInfo {
    pub name: String,
    pub active: bool,
    pub path: String,
    pub controller_url: Option<String>,
    pub controller_changed: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct ProfileDetail {
    pub name: String,
    pub active: bool,
    pub path: String,
    pub content: String,
}

pub fn profile_to_info(profile: MihomoProfile) -> ProfileInfo {
    ProfileInfo {
        name: profile.name,
        active: profile.active,
        path: profile.path.to_string_lossy().to_string(),
        controller_url: None,
        controller_changed: None,
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

pub async fn load_profile_detail(name: &str) -> anyhow::Result<ProfileDetail> {
    let profile = load_profile_info(name).await?;
    let manager = ConfigManager::new()?;
    let content = manager.load(&profile.name).await?;
    Ok(ProfileDetail {
        name: profile.name,
        active: profile.active,
        path: profile.path,
        content,
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
