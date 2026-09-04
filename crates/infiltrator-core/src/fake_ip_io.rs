//! Current-profile filesystem adapter for the domain Fake-IP configuration.

use anyhow::Context;
use infiltrator_domain::fake_ip::{
    FakeIpConfig, FakeIpConfigPatch, apply_fake_ip_patch_to_yaml, extract_fake_ip_config_from_doc,
};
use serde_yaml_ng::Value;

pub async fn load_fake_ip_config() -> anyhow::Result<FakeIpConfig> {
    let manager = crate::settings_io::app_config_manager()
        .await
        .context("init config manager")?;
    let profile = manager
        .get_current()
        .await
        .context("load current profile")?;
    let content = manager
        .load(&profile)
        .await
        .context("read profile config")?;
    let doc: Value = serde_yaml_ng::from_str(&content).context("parse profile yaml")?;
    extract_fake_ip_config_from_doc(&doc)
}

pub async fn save_fake_ip_config(patch: FakeIpConfigPatch) -> anyhow::Result<FakeIpConfig> {
    let manager = crate::settings_io::app_config_manager()
        .await
        .context("init config manager")?;
    let profile = manager
        .get_current()
        .await
        .context("load current profile")?;
    let content = manager
        .load(&profile)
        .await
        .context("read profile config")?;
    let updated = apply_fake_ip_patch_to_yaml(&content, patch)?;
    let doc: Value = serde_yaml_ng::from_str(&updated).context("parse updated profile yaml")?;
    let config = extract_fake_ip_config_from_doc(&doc)?;
    manager
        .save(&profile, &updated)
        .await
        .context("save profile config")?;
    Ok(config)
}

pub async fn clear_fake_ip_cache() -> anyhow::Result<bool> {
    let manager = crate::settings_io::app_config_manager()
        .await
        .context("init config manager")?;
    let profile_path = manager
        .get_current_path()
        .await
        .context("load current profile path")?;
    let config_dir = profile_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("profile path has no parent directory"))?;
    let cache_path = config_dir.join("fake-ip-cache");
    if tokio::fs::try_exists(&cache_path)
        .await
        .context("check fake-ip cache")?
    {
        tokio::fs::remove_file(&cache_path)
            .await
            .context("remove fake-ip cache")?;
        return Ok(true);
    }
    Ok(false)
}
