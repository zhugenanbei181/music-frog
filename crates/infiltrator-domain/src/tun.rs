use anyhow::{Context, Result, anyhow};
use infiltrator_contract::tun::{TunDnsMode, TunDnsModeUpdate, TunStack, TunStackUpdate};
use serde::{Deserialize, Serialize};
use serde_yaml_ng::{Mapping, Value};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct TunConfig {
    pub enable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stack: Option<String>,
    pub dns_mode: Option<TunDnsMode>,
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
    pub stack: TunStackUpdate,
    #[serde(default)]
    pub dns_mode: TunDnsModeUpdate,
    pub dns_hijack: Option<Vec<String>>,
    pub auto_route: Option<bool>,
    pub auto_detect_interface: Option<bool>,
    pub mtu: Option<u32>,
    pub strict_route: Option<bool>,
}

impl TunConfig {
    pub fn apply_patch(&mut self, patch: TunConfigPatch) {
        if let Some(value) = patch.enable {
            self.enable = Some(value);
        }
        match patch.stack {
            TunStackUpdate::Keep => {}
            TunStackUpdate::Clear => self.stack = None,
            TunStackUpdate::Set(value) => self.stack = Some(value.as_str().to_owned()),
        }
        match patch.dns_mode {
            TunDnsModeUpdate::Keep => {}
            TunDnsModeUpdate::Clear => self.dns_mode = None,
            TunDnsModeUpdate::Set(value) => self.dns_mode = Some(value),
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
            && self.dns_mode.is_none()
            && self.dns_hijack.is_none()
            && self.auto_route.is_none()
            && self.auto_detect_interface.is_none()
            && self.mtu.is_none()
            && self.strict_route.is_none()
    }
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
        let parsed = TunStack::parse(stack);
        if parsed.is_none_or(|value| !value.is_live_supported()) {
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
    fn dns_mode_rejects_unknown_persisted_values() {
        let doc: Value = serde_yaml_ng::from_str("tun:\n  dns-mode: automatic\n").expect("yaml");
        assert!(extract_tun_config_from_doc(&doc).is_err());
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
            stack: TunStackUpdate::Set(TunStack::Gvisor),
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
            // Stack defaults to Keep; mtu is also left untouched.
            ..TunConfigPatch::default()
        };

        config.apply_patch(patch);

        assert_eq!(config.enable, Some(false));
        // Should preserve existing values
        assert_eq!(config.stack, Some("system".to_string()));
        assert_eq!(config.mtu, Some(1400));
    }

    #[test]
    fn stack_keep_clear_and_set_have_distinct_persisted_effects() {
        let mut config = TunConfig {
            stack: Some("system".to_owned()),
            mtu: Some(1420),
            ..TunConfig::default()
        };
        config.apply_patch(TunConfigPatch {
            stack: TunStackUpdate::Keep,
            ..TunConfigPatch::default()
        });
        assert_eq!(config.stack.as_deref(), Some("system"));

        config.apply_patch(TunConfigPatch {
            stack: TunStackUpdate::Clear,
            ..TunConfigPatch::default()
        });
        assert_eq!(config.stack, None);

        let yaml = apply_tun_patch_to_yaml(
            "tun:\n  enable: true\n  stack: system\n",
            TunConfigPatch {
                stack: TunStackUpdate::Clear,
                ..TunConfigPatch::default()
            },
        )
        .expect("clear stack from profile");
        let document: Value = serde_yaml_ng::from_str(&yaml).expect("valid updated YAML");
        let tun = document
            .get("tun")
            .and_then(Value::as_mapping)
            .expect("tun mapping");
        assert!(tun.get(Value::String("stack".to_owned())).is_none());
        assert_eq!(
            tun.get(Value::String("enable".to_owned()))
                .and_then(Value::as_bool),
            Some(true),
        );
    }

    #[test]
    fn dns_mode_is_preserved_and_has_distinct_keep_clear_set_effects() {
        let mut config = TunConfig {
            dns_mode: Some(TunDnsMode::Hijack),
            ..TunConfig::default()
        };
        config.apply_patch(TunConfigPatch {
            dns_mode: TunDnsModeUpdate::Keep,
            ..TunConfigPatch::default()
        });
        assert_eq!(config.dns_mode, Some(TunDnsMode::Hijack));

        config.apply_patch(TunConfigPatch {
            dns_mode: TunDnsModeUpdate::Set(TunDnsMode::Native),
            ..TunConfigPatch::default()
        });
        assert_eq!(config.dns_mode, Some(TunDnsMode::Native));

        config.apply_patch(TunConfigPatch {
            dns_mode: TunDnsModeUpdate::Clear,
            ..TunConfigPatch::default()
        });
        assert_eq!(config.dns_mode, None);
    }

    #[test]
    fn unrelated_tun_patch_does_not_drop_dns_mode_from_the_document() {
        let yaml = apply_tun_patch_to_yaml(
            "tun:\n  enable: true\n  dns-mode: hijack\n",
            TunConfigPatch {
                mtu: Some(1400),
                ..TunConfigPatch::default()
            },
        )
        .expect("preserve dns-mode");
        let document: Value = serde_yaml_ng::from_str(&yaml).expect("valid updated YAML");
        let tun = document
            .get("tun")
            .and_then(Value::as_mapping)
            .expect("tun mapping");
        assert_eq!(
            tun.get(Value::String("dns-mode".to_owned()))
                .and_then(Value::as_str),
            Some("hijack")
        );
    }

    #[test]
    fn test_validate_tun_accepts_all_current_mihomo_stacks() {
        for stack in ["gvisor", "system", "mixed"] {
            let config = TunConfig {
                stack: Some(stack.to_owned()),
                ..TunConfig::default()
            };
            assert!(validate_tun_config(&config).is_ok(), "{stack}");
        }
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
}
