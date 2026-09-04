//! Current-profile filesystem adapter for sniffer configuration.

use anyhow::{Context, Result};
use infiltrator_domain::sniffer::{
    SnifferConfig, apply_sniffer_config, apply_typed_sniffer_config, extract_sniffer_config,
    extract_sniffer_config_from_doc, validate_sniffer_config, validate_typed_sniffer_config,
};
use serde_yaml_ng::Value;

use crate::settings_io::app_config_manager;

/// Load current profile's sniffer config as JSON.
pub async fn load_sniffer_config() -> Result<serde_json::Value> {
    let doc = load_profile_doc().await?;
    extract_sniffer_config_from_doc(&doc)
}

/// Save current profile's sniffer config from JSON.
pub async fn save_sniffer_config(config: serde_json::Value) -> Result<serde_json::Value> {
    validate_sniffer_config(&config)?;
    let manager = app_config_manager().await.context("init config manager")?;
    let profile = manager
        .get_current()
        .await
        .context("load current profile")?;
    let content = manager
        .load(&profile)
        .await
        .context("read profile config")?;
    let mut doc: Value = serde_yaml_ng::from_str(&content).context("parse profile yaml")?;

    apply_sniffer_config(&mut doc, &config)?;

    let updated = serde_yaml_ng::to_string(&doc).context("serialize profile yaml")?;
    manager
        .save(&profile, &updated)
        .await
        .context("save profile config")?;
    Ok(config)
}

/// Load current profile's typed sniffer config.
pub async fn load_typed_sniffer_config() -> Result<SnifferConfig> {
    let doc = load_profile_doc().await?;
    extract_sniffer_config(&doc)
}

/// Save current profile's typed sniffer config.
pub async fn save_typed_sniffer_config(config: SnifferConfig) -> Result<SnifferConfig> {
    validate_typed_sniffer_config(&config)?;
    let manager = app_config_manager().await.context("init config manager")?;
    let profile = manager
        .get_current()
        .await
        .context("load current profile")?;
    let content = manager
        .load(&profile)
        .await
        .context("read profile config")?;
    let mut doc: Value = serde_yaml_ng::from_str(&content).context("parse profile yaml")?;

    apply_typed_sniffer_config(&mut doc, &config)?;

    let updated = serde_yaml_ng::to_string(&doc).context("serialize profile yaml")?;
    manager
        .save(&profile, &updated)
        .await
        .context("save profile config")?;
    Ok(config)
}

async fn load_profile_doc() -> Result<Value> {
    let manager = app_config_manager().await.context("init config manager")?;
    let profile = manager
        .get_current()
        .await
        .context("load current profile")?;
    let content = manager
        .load(&profile)
        .await
        .context("read profile config")?;
    serde_yaml_ng::from_str(&content).context("parse profile yaml")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// sniffer 读写必须落在 settings `configs_dir` 重定向后的目录：
    /// 重定向态下读回成功即证明写入落点正确，`<home>/configs` 保持未创建。
    #[tokio::test]
    async fn test_sniffer_io_follows_configs_dir_redirect() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().to_path_buf();
        let cloud = home.join("cloud-sync").join("profiles");
        std::fs::create_dir_all(&cloud).unwrap();
        let guard = crate::settings_io::test_support::RedirectGuard::acquire(home.clone()).await;
        guard
            .set_configs_dir(&home, Some(cloud.to_str().unwrap()))
            .await;

        let seed = crate::settings_io::app_config_manager().await.unwrap();
        seed.save("main", "port: 7890\n").await.unwrap();
        seed.set_current("main").await.unwrap();

        let saved = save_sniffer_config(serde_json::json!({"enable": true}))
            .await
            .unwrap();
        assert_eq!(saved, serde_json::json!({"enable": true}));

        let loaded = load_sniffer_config().await.unwrap();
        assert_eq!(loaded, serde_json::json!({"enable": true}));
        assert!(cloud.join("main.yaml").is_file());
        assert!(!home.join("configs").exists());
    }
}
