//! Current-profile filesystem adapter for proxy-provider configuration.

use anyhow::{Context, Result};
use infiltrator_domain::proxy_providers::{
    ProxyProviders, apply_proxy_providers_to_yaml, extract_proxy_providers_from_doc,
};
use serde_yaml_ng::Value;

use crate::settings_io::app_config_manager;

pub async fn load_proxy_providers() -> Result<ProxyProviders> {
    let manager = app_config_manager().await.context("init config manager")?;
    let profile = manager
        .get_current()
        .await
        .context("load current profile")?;
    let content = manager
        .load(&profile)
        .await
        .context("read profile config")?;
    let doc: Value = serde_yaml_ng::from_str(&content).context("parse profile yaml")?;
    extract_proxy_providers_from_doc(&doc)
}

pub async fn save_proxy_providers(providers: ProxyProviders) -> Result<ProxyProviders> {
    let manager = app_config_manager().await.context("init config manager")?;
    let profile = manager
        .get_current()
        .await
        .context("load current profile")?;
    let content = manager
        .load(&profile)
        .await
        .context("read profile config")?;
    let updated = apply_proxy_providers_to_yaml(&content, &providers)?;
    manager
        .save(&profile, &updated)
        .await
        .context("save profile config")?;
    Ok(providers)
}
