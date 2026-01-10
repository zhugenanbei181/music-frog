use anyhow::{anyhow, Context, Result};
use mihomo_config::ConfigManager;
use serde::{Deserialize, Serialize};
use serde_yaml::{Mapping, Value};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct TunConfig {
    pub enable: Option<bool>,
    pub stack: Option<String>,
    pub dns_hijack: Option<Vec<String>>,
    pub auto_route: Option<bool>,
    pub auto_detect_interface: Option<bool>,
    pub mtu: Option<u32>,
    pub strict_route: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct TunConfigPatch {
    pub enable: Option<bool>,
    pub stack: Option<String>,
    pub dns_hijack: Option<Vec<String>>,
    pub auto_route: Option<bool>,
    pub auto_detect_interface: Option<bool>,
    pub mtu: Option<u32>,
    pub strict_route: Option<bool>,
}

impl TunConfig {
    fn apply_patch(&mut self, patch: TunConfigPatch) {
        if let Some(value) = patch.enable {
            self.enable = Some(value);
        }
        if let Some(value) = patch.stack {
            self.stack = Some(value);
        }
        if let Some(value) = patch.dns_hijack {
            self.dns_hijack = Some(value);
        }
        if let Some(value) = patch.auto_route {
            self.auto_route = Some(value);
        }
        if let Some(value) = patch.auto_detect_interface {
            self.auto_detect_interface = Some(value);
        }
        if let Some(value) = patch.mtu {
            self.mtu = Some(value);
        }
        if let Some(value) = patch.strict_route {
            self.strict_route = Some(value);
        }
    }
}

pub async fn load_tun_config() -> Result<TunConfig> {
    let doc = load_profile_doc().await?;
    extract_tun_config(&doc)
}

pub async fn save_tun_config(patch: TunConfigPatch) -> Result<TunConfig> {
    let manager = ConfigManager::new().context("init config manager")?;
    let profile = manager.get_current().await.context("load current profile")?;
    let content = manager.load(&profile).await.context("read profile config")?;
    let mut doc: Value = serde_yaml::from_str(&content).context("parse profile yaml")?;

    let mut config = extract_tun_config(&doc)?;
    config.apply_patch(patch);
    validate_tun_config(&config)?;
    apply_tun_config(&mut doc, &config)?;

    let updated = serde_yaml::to_string(&doc).context("serialize profile yaml")?;
    manager
        .save(&profile, &updated)
        .await
        .context("save profile config")?;
    Ok(config)
}

async fn load_profile_doc() -> Result<Value> {
    let manager = ConfigManager::new().context("init config manager")?;
    let profile = manager.get_current().await.context("load current profile")?;
    let content = manager.load(&profile).await.context("read profile config")?;
    serde_yaml::from_str(&content).context("parse profile yaml")
}

fn extract_tun_config(doc: &Value) -> Result<TunConfig> {
    let value = doc
        .get("tun")
        .cloned()
        .unwrap_or(Value::Mapping(Mapping::new()));
    let config = serde_yaml::from_value(value).context("decode tun config")?;
    Ok(config)
}

fn apply_tun_config(doc: &mut Value, config: &TunConfig) -> Result<()> {
    let map = doc
        .as_mapping_mut()
        .ok_or_else(|| anyhow!("profile config is not a mapping"))?;
    let tun_value = serde_yaml::to_value(config).context("encode tun config")?;
    if tun_value == Value::Mapping(Mapping::new()) {
        map.remove(&Value::String("tun".to_string()));
        return Ok(());
    }
    map.insert(Value::String("tun".to_string()), tun_value);
    Ok(())
}

fn validate_tun_config(config: &TunConfig) -> Result<()> {
    if let Some(stack) = config.stack.as_ref() {
        let lower = stack.trim().to_ascii_lowercase();
        if lower != "system" && lower != "gvisor" {
            return Err(anyhow!("unsupported tun stack: {}", stack));
        }
    }
    if let Some(dns_hijack) = config.dns_hijack.as_ref() {
        for entry in dns_hijack {
            if entry.trim().is_empty() {
                return Err(anyhow!("dns-hijack contains empty entry"));
            }
        }
    }
    if let Some(mtu) = config.mtu {
        if mtu == 0 {
            return Err(anyhow!("mtu must be greater than 0"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_tun_default() {
        let doc: Value = serde_yaml::from_str("port: 7890\n").expect("yaml");
        let config = extract_tun_config(&doc).expect("tun config");
        assert!(config.enable.is_none());
    }

    #[test]
    fn test_validate_tun_rejects_stack() {
        let config = TunConfig {
            stack: Some("invalid".to_string()),
            ..TunConfig::default()
        };
        assert!(validate_tun_config(&config).is_err());
    }

    #[test]
    fn test_apply_tun_config_removes_empty() {
        let mut doc: Value = serde_yaml::from_str("tun:\n  enable: true\n").expect("yaml");
        let config = TunConfig::default();
        apply_tun_config(&mut doc, &config).expect("apply tun");
        let map = doc.as_mapping().expect("mapping");
        assert!(map.get(&Value::String("tun".to_string())).is_none());
    }
}
