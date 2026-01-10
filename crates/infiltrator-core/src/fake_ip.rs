use anyhow::{anyhow, Context, Result};
use mihomo_config::ConfigManager;
use serde::{Deserialize, Serialize};
use serde_yaml::{Mapping, Value};
use tokio::fs;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct FakeIpConfig {
    pub fake_ip_range: Option<String>,
    pub fake_ip_filter: Option<Vec<String>>,
    pub store_fake_ip: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct FakeIpConfigPatch {
    pub fake_ip_range: Option<String>,
    pub fake_ip_filter: Option<Vec<String>>,
    pub store_fake_ip: Option<bool>,
}

impl FakeIpConfig {
    fn apply_patch(&mut self, patch: FakeIpConfigPatch) {
        if let Some(value) = patch.fake_ip_range {
            self.fake_ip_range = Some(value);
        }
        if let Some(value) = patch.fake_ip_filter {
            self.fake_ip_filter = Some(value);
        }
        if let Some(value) = patch.store_fake_ip {
            self.store_fake_ip = Some(value);
        }
    }
}

pub async fn load_fake_ip_config() -> Result<FakeIpConfig> {
    let manager = ConfigManager::new().context("init config manager")?;
    let profile = manager.get_current().await.context("load current profile")?;
    let content = manager.load(&profile).await.context("read profile config")?;
    let doc: Value = serde_yaml::from_str(&content).context("parse profile yaml")?;
    extract_fake_ip_config(&doc)
}

pub async fn save_fake_ip_config(patch: FakeIpConfigPatch) -> Result<FakeIpConfig> {
    let manager = ConfigManager::new().context("init config manager")?;
    let profile = manager.get_current().await.context("load current profile")?;
    let content = manager.load(&profile).await.context("read profile config")?;
    let mut doc: Value = serde_yaml::from_str(&content).context("parse profile yaml")?;

    let mut config = extract_fake_ip_config(&doc)?;
    config.apply_patch(patch);
    validate_fake_ip_config(&config)?;
    apply_fake_ip_config(&mut doc, &config)?;

    let updated = serde_yaml::to_string(&doc).context("serialize profile yaml")?;
    manager
        .save(&profile, &updated)
        .await
        .context("save profile config")?;
    Ok(config)
}

pub async fn clear_fake_ip_cache() -> Result<bool> {
    let manager = ConfigManager::new().context("init config manager")?;
    let profile_path = manager.get_current_path().await.context("load current profile path")?;
    let config_dir = profile_path
        .parent()
        .ok_or_else(|| anyhow!("profile path has no parent directory"))?;
    let cache_path = config_dir.join("fake-ip-cache");
    if fs::try_exists(&cache_path).await.context("check fake-ip cache")? {
        fs::remove_file(&cache_path)
            .await
            .context("remove fake-ip cache")?;
        return Ok(true);
    }
    Ok(false)
}

fn extract_fake_ip_config(doc: &Value) -> Result<FakeIpConfig> {
    let dns_value = doc
        .get("dns")
        .cloned()
        .unwrap_or(Value::Mapping(Mapping::new()));
    let config = serde_yaml::from_value(dns_value).context("decode fake-ip config")?;
    Ok(config)
}

fn apply_fake_ip_config(doc: &mut Value, config: &FakeIpConfig) -> Result<()> {
    let map = doc
        .as_mapping_mut()
        .ok_or_else(|| anyhow!("profile config is not a mapping"))?;
    let dns_entry = map
        .entry(Value::String("dns".to_string()))
        .or_insert_with(|| Value::Mapping(Mapping::new()));
    let dns_map = dns_entry
        .as_mapping_mut()
        .ok_or_else(|| anyhow!("dns section is not a mapping"))?;

    if let Some(range) = config.fake_ip_range.as_ref() {
        dns_map.insert(
            Value::String("fake-ip-range".to_string()),
            Value::String(range.clone()),
        );
    }
    if let Some(filter) = config.fake_ip_filter.as_ref() {
        let value = serde_yaml::to_value(filter).context("encode fake-ip-filter")?;
        dns_map.insert(Value::String("fake-ip-filter".to_string()), value);
    }
    if let Some(store) = config.store_fake_ip {
        dns_map.insert(
            Value::String("store-fake-ip".to_string()),
            Value::Bool(store),
        );
    }
    Ok(())
}

fn validate_fake_ip_config(config: &FakeIpConfig) -> Result<()> {
    if let Some(range) = config.fake_ip_range.as_ref() {
        if range.trim().is_empty() {
            return Err(anyhow!("fake-ip-range is empty"));
        }
    }
    if let Some(filter) = config.fake_ip_filter.as_ref() {
        for entry in filter {
            if entry.trim().is_empty() {
                return Err(anyhow!("fake-ip-filter contains empty entry"));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_fake_ip_default() {
        let doc: Value = serde_yaml::from_str("port: 7890\n").expect("yaml");
        let config = extract_fake_ip_config(&doc).expect("fake ip config");
        assert!(config.fake_ip_range.is_none());
    }

    #[test]
    fn test_apply_patch_and_validate() {
        let doc: Value = serde_yaml::from_str("port: 7890\n").expect("yaml");
        let mut config = extract_fake_ip_config(&doc).expect("fake ip config");
        let patch = FakeIpConfigPatch {
            fake_ip_range: Some("198.18.0.1/16".to_string()),
            fake_ip_filter: Some(vec!["*.lan".to_string()]),
            store_fake_ip: Some(true),
        };
        config.apply_patch(patch);
        validate_fake_ip_config(&config).expect("valid fake-ip config");
    }

    #[test]
    fn test_apply_fake_ip_config_updates_dns() {
        let mut doc: Value = serde_yaml::from_str("port: 7890\n").expect("yaml");
        let config = FakeIpConfig {
            fake_ip_range: Some("198.18.0.1/16".to_string()),
            ..FakeIpConfig::default()
        };
        apply_fake_ip_config(&mut doc, &config).expect("apply fake ip");
        let map = doc.as_mapping().expect("mapping");
        let dns = map.get(&Value::String("dns".to_string()));
        assert!(dns.is_some());
    }

    #[test]
    fn test_validate_rejects_empty_filter_entry() {
        let config = FakeIpConfig {
            fake_ip_filter: Some(vec!["".to_string()]),
            ..FakeIpConfig::default()
        };
        assert!(validate_fake_ip_config(&config).is_err());
    }
}
