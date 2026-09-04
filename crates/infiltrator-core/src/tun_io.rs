//! Current-profile filesystem adapter for the domain TUN configuration.

use anyhow::Context;
use infiltrator_domain::tun::{TunConfig, TunConfigPatch, apply_tun_patch_to_yaml, extract_tun_config_from_doc};
use serde_yaml_ng::Value;

pub async fn load_tun_config() -> anyhow::Result<TunConfig> {
    let manager = crate::settings::app_config_manager()
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
    extract_tun_config_from_doc(&doc)
}

pub async fn save_tun_config(patch: TunConfigPatch) -> anyhow::Result<TunConfig> {
    let manager = crate::settings::app_config_manager()
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
    let updated = apply_tun_patch_to_yaml(&content, patch)?;
    let doc: Value = serde_yaml_ng::from_str(&updated).context("parse updated profile yaml")?;
    let config = extract_tun_config_from_doc(&doc)?;
    manager
        .save(&profile, &updated)
        .await
        .context("save profile config")?;
    Ok(config)
}

#[cfg(test)]
#[path = "tun_io_test.rs"]
mod tests;
