//! Current-profile filesystem adapter for the domain DNS configuration.

use anyhow::Context;
use infiltrator_domain::dns::{DnsConfig, DnsConfigPatch, apply_dns_patch_to_yaml, extract_dns_config_from_doc};
use serde_yaml_ng::Value;

pub async fn load_dns_config() -> anyhow::Result<DnsConfig> {
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
    extract_dns_config_from_doc(&doc)
}

pub async fn save_dns_config(patch: DnsConfigPatch) -> anyhow::Result<DnsConfig> {
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
    let updated = apply_dns_patch_to_yaml(&content, patch)?;
    let doc: Value = serde_yaml_ng::from_str(&updated).context("parse updated profile yaml")?;
    let config = extract_dns_config_from_doc(&doc)?;
    manager
        .save(&profile, &updated)
        .await
        .context("save profile config")?;
    Ok(config)
}

#[cfg(test)]
#[path = "dns_io_test.rs"]
mod tests;
