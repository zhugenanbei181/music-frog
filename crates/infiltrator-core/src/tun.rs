use crate::settings::app_config_manager;
use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use serde_yaml_ng::{Mapping, Value};

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

    fn is_empty(&self) -> bool {
        self.enable.is_none()
            && self.stack.is_none()
            && self.dns_hijack.is_none()
            && self.auto_route.is_none()
            && self.auto_detect_interface.is_none()
            && self.mtu.is_none()
            && self.strict_route.is_none()
    }
}

pub async fn load_tun_config() -> Result<TunConfig> {
    let doc = load_profile_doc().await?;
    extract_tun_config_from_doc(&doc)
}

pub async fn save_tun_config(patch: TunConfigPatch) -> Result<TunConfig> {
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

    let mut config = extract_tun_config_from_doc(&doc)?;
    config.apply_patch(patch);
    validate_tun_config(&config)?;
    apply_tun_config(&mut doc, &config)?;

    let updated = serde_yaml_ng::to_string(&doc).context("serialize profile yaml")?;
    manager
        .save(&profile, &updated)
        .await
        .context("save profile config")?;
    Ok(config)
}

/// Apply a TUN patch to an in-memory profile document for the shared atomic
/// Apply transaction.
pub fn apply_tun_patch_to_yaml(content: &str, patch: TunConfigPatch) -> Result<String> {
    let mut doc: Value = serde_yaml_ng::from_str(content).context("parse profile yaml")?;
    let mut config = extract_tun_config_from_doc(&doc)?;
    config.apply_patch(patch);
    validate_tun_config(&config)?;
    apply_tun_config(&mut doc, &config)?;
    serde_yaml_ng::to_string(&doc).context("serialize profile yaml")
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

pub fn extract_tun_config_from_doc(doc: &Value) -> Result<TunConfig> {
    let value = doc
        .get("tun")
        .cloned()
        .unwrap_or(Value::Mapping(Mapping::new()));
    let config = serde_yaml_ng::from_value(value).context("decode tun config")?;
    Ok(config)
}

fn apply_tun_config(doc: &mut Value, config: &TunConfig) -> Result<()> {
    let map = doc
        .as_mapping_mut()
        .ok_or_else(|| anyhow!("profile config is not a mapping"))?;
    if config.is_empty() {
        map.remove(Value::String("tun".to_string()));
        return Ok(());
    }
    let tun_value = serde_yaml_ng::to_value(config).context("encode tun config")?;
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
    if let Some(mtu) = config.mtu
        && mtu == 0
    {
        return Err(anyhow!("mtu must be greater than 0"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_tun_default() {
        let doc: Value = serde_yaml_ng::from_str("port: 7890\n").expect("yaml");
        let config = extract_tun_config_from_doc(&doc).expect("tun config");
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
        let mut doc: Value = serde_yaml_ng::from_str("tun:\n  enable: true\n").expect("yaml");
        let config = TunConfig::default();
        apply_tun_config(&mut doc, &config).expect("apply tun");
        let map = doc.as_mapping().expect("mapping");
        assert!(map.get(Value::String("tun".to_string())).is_none());
    }

    #[test]
    fn test_apply_patch() {
        let mut config = TunConfig::default();
        let patch = TunConfigPatch {
            enable: Some(true),
            stack: Some("gvisor".to_string()),
            mtu: Some(1500),
            ..TunConfigPatch::default()
        };
        config.apply_patch(patch);
        assert_eq!(config.enable, Some(true));
        assert_eq!(config.stack, Some("gvisor".to_string()));
        assert_eq!(config.mtu, Some(1500));
    }

    #[test]
    fn test_apply_partial_patch_preserves_existing() {
        let mut config = TunConfig {
            enable: Some(true),
            stack: Some("system".to_string()),
            mtu: Some(1400),
            ..TunConfig::default()
        };

        let patch = TunConfigPatch {
            enable: Some(false),
            // stack and mtu are None in patch
            ..TunConfigPatch::default()
        };

        config.apply_patch(patch);

        assert_eq!(config.enable, Some(false));
        // Should preserve existing values
        assert_eq!(config.stack, Some("system".to_string()));
        assert_eq!(config.mtu, Some(1400));
    }

    #[test]
    fn test_validate_tun_config_errors() {
        // Invalid stack
        let config = TunConfig {
            stack: Some("lwip".to_string()),
            ..TunConfig::default()
        };
        assert!(validate_tun_config(&config).is_err());

        // Zero MTU
        let config = TunConfig {
            mtu: Some(0),
            ..TunConfig::default()
        };
        assert!(validate_tun_config(&config).is_err());

        // Empty dns_hijack
        let config = TunConfig {
            dns_hijack: Some(vec![" ".to_string()]),
            ..TunConfig::default()
        };
        assert!(validate_tun_config(&config).is_err());
    }

    /// tun 读写必须落在 settings `configs_dir` 重定向后的目录：
    /// 重定向态下读回成功即证明写入落点正确，`<home>/configs` 保持未创建。
    #[tokio::test]
    async fn test_tun_io_follows_configs_dir_redirect() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().to_path_buf();
        let cloud = home.join("cloud-sync").join("profiles");
        std::fs::create_dir_all(&cloud).unwrap();
        let guard = crate::settings::test_support::RedirectGuard::acquire(home.clone()).await;
        guard
            .set_configs_dir(&home, Some(cloud.to_str().unwrap()))
            .await;

        let seed = crate::settings::app_config_manager().await.unwrap();
        seed.save("main", "port: 7890\n").await.unwrap();
        seed.set_current("main").await.unwrap();

        let saved = save_tun_config(TunConfigPatch {
            enable: Some(true),
            ..TunConfigPatch::default()
        })
        .await
        .unwrap();
        assert_eq!(saved.enable, Some(true));

        let loaded = load_tun_config().await.unwrap();
        assert_eq!(loaded.enable, Some(true));
        assert!(cloud.join("main.yaml").is_file());
        assert!(!home.join("configs").exists());
    }
}
