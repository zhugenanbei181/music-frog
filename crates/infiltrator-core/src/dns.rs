use anyhow::{anyhow, Context, Result};
use mihomo_config::ConfigManager;
use serde::{Deserialize, Serialize};
use serde_yaml::{Mapping, Value};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct DnsFallbackFilter {
    pub geoip: Option<bool>,
    pub geoip_code: Option<String>,
    pub ipcidr: Option<Vec<String>>,
    pub domain: Option<Vec<String>>,
    pub domain_suffix: Option<Vec<String>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct DnsConfig {
    pub enable: Option<bool>,
    pub ipv6: Option<bool>,
    pub listen: Option<String>,
    pub default_nameserver: Option<Vec<String>>,
    pub nameserver: Option<Vec<String>>,
    pub fallback: Option<Vec<String>>,
    pub fallback_filter: Option<DnsFallbackFilter>,
    pub enhanced_mode: Option<String>,
    pub fake_ip_range: Option<String>,
    pub fake_ip_filter: Option<Vec<String>>,
    pub use_hosts: Option<bool>,
    pub use_system_hosts: Option<bool>,
    pub respect_rules: Option<bool>,
    pub proxy_server_nameserver: Option<Vec<String>>,
    pub direct_nameserver: Option<Vec<String>>,
    pub cache: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct DnsConfigPatch {
    pub enable: Option<bool>,
    pub ipv6: Option<bool>,
    pub listen: Option<String>,
    pub default_nameserver: Option<Vec<String>>,
    pub nameserver: Option<Vec<String>>,
    pub fallback: Option<Vec<String>>,
    pub fallback_filter: Option<DnsFallbackFilter>,
    pub enhanced_mode: Option<String>,
    pub fake_ip_range: Option<String>,
    pub fake_ip_filter: Option<Vec<String>>,
    pub use_hosts: Option<bool>,
    pub use_system_hosts: Option<bool>,
    pub respect_rules: Option<bool>,
    pub proxy_server_nameserver: Option<Vec<String>>,
    pub direct_nameserver: Option<Vec<String>>,
    pub cache: Option<bool>,
}

impl DnsConfig {
    fn apply_patch(&mut self, patch: DnsConfigPatch) {
        if let Some(value) = patch.enable {
            self.enable = Some(value);
        }
        if let Some(value) = patch.ipv6 {
            self.ipv6 = Some(value);
        }
        if let Some(value) = patch.listen {
            self.listen = Some(value);
        }
        if let Some(value) = patch.default_nameserver {
            self.default_nameserver = Some(value);
        }
        if let Some(value) = patch.nameserver {
            self.nameserver = Some(value);
        }
        if let Some(value) = patch.fallback {
            self.fallback = Some(value);
        }
        if let Some(value) = patch.fallback_filter {
            self.fallback_filter = Some(value);
        }
        if let Some(value) = patch.enhanced_mode {
            self.enhanced_mode = Some(value);
        }
        if let Some(value) = patch.fake_ip_range {
            self.fake_ip_range = Some(value);
        }
        if let Some(value) = patch.fake_ip_filter {
            self.fake_ip_filter = Some(value);
        }
        if let Some(value) = patch.use_hosts {
            self.use_hosts = Some(value);
        }
        if let Some(value) = patch.use_system_hosts {
            self.use_system_hosts = Some(value);
        }
        if let Some(value) = patch.respect_rules {
            self.respect_rules = Some(value);
        }
        if let Some(value) = patch.proxy_server_nameserver {
            self.proxy_server_nameserver = Some(value);
        }
        if let Some(value) = patch.direct_nameserver {
            self.direct_nameserver = Some(value);
        }
        if let Some(value) = patch.cache {
            self.cache = Some(value);
        }
    }

    fn is_empty(&self) -> bool {
        self.enable.is_none()
            && self.ipv6.is_none()
            && self.listen.is_none()
            && self.default_nameserver.is_none()
            && self.nameserver.is_none()
            && self.fallback.is_none()
            && self.fallback_filter.is_none()
            && self.enhanced_mode.is_none()
            && self.fake_ip_range.is_none()
            && self.fake_ip_filter.is_none()
            && self.use_hosts.is_none()
            && self.use_system_hosts.is_none()
            && self.respect_rules.is_none()
            && self.proxy_server_nameserver.is_none()
            && self.direct_nameserver.is_none()
            && self.cache.is_none()
    }
}

pub async fn load_dns_config() -> Result<DnsConfig> {
    let manager = ConfigManager::new().context("init config manager")?;
    let profile = manager.get_current().await.context("load current profile")?;
    let content = manager.load(&profile).await.context("read profile config")?;
    let doc: Value = serde_yaml::from_str(&content).context("parse profile yaml")?;
    extract_dns_config(&doc)
}

pub async fn save_dns_config(patch: DnsConfigPatch) -> Result<DnsConfig> {
    let manager = ConfigManager::new().context("init config manager")?;
    let profile = manager.get_current().await.context("load current profile")?;
    let content = manager.load(&profile).await.context("read profile config")?;
    let mut doc: Value = serde_yaml::from_str(&content).context("parse profile yaml")?;

    let mut config = extract_dns_config(&doc)?;
    config.apply_patch(patch);
    validate_dns_config(&config)?;
    apply_dns_config(&mut doc, &config)?;

    let updated = serde_yaml::to_string(&doc).context("serialize profile yaml")?;
    manager
        .save(&profile, &updated)
        .await
        .context("save profile config")?;
    Ok(config)
}

fn extract_dns_config(doc: &Value) -> Result<DnsConfig> {
    let dns_value = doc
        .get("dns")
        .cloned()
        .unwrap_or(Value::Mapping(Mapping::new()));
    let config = serde_yaml::from_value(dns_value).context("decode dns config")?;
    Ok(config)
}

fn apply_dns_config(doc: &mut Value, config: &DnsConfig) -> Result<()> {
    let map = doc
        .as_mapping_mut()
        .ok_or_else(|| anyhow!("profile config is not a mapping"))?;
    if config.is_empty() {
        map.remove(&Value::String("dns".to_string()));
        return Ok(());
    }
    let dns_value = serde_yaml::to_value(config).context("encode dns config")?;
    map.insert(Value::String("dns".to_string()), dns_value);
    Ok(())
}

fn validate_dns_config(config: &DnsConfig) -> Result<()> {
    validate_list("default-nameserver", &config.default_nameserver)?;
    validate_list("nameserver", &config.nameserver)?;
    validate_list("fallback", &config.fallback)?;
    validate_list("proxy-server-nameserver", &config.proxy_server_nameserver)?;
    validate_list("direct-nameserver", &config.direct_nameserver)?;

    if let Some(mode) = config.enhanced_mode.as_ref() {
        let mode = mode.trim().to_ascii_lowercase();
        if mode != "fake-ip" && mode != "redir-host" {
            return Err(anyhow!("unsupported enhanced-mode: {}", mode));
        }
    }
    if let Some(listen) = config.listen.as_ref() {
        if listen.trim().is_empty() {
            return Err(anyhow!("dns listen address is empty"));
        }
    }
    if let Some(range) = config.fake_ip_range.as_ref() {
        if range.trim().is_empty() {
            return Err(anyhow!("fake-ip-range is empty"));
        }
    }
    Ok(())
}

fn validate_list(label: &str, list: &Option<Vec<String>>) -> Result<()> {
    if let Some(items) = list.as_ref() {
        for item in items {
            if item.trim().is_empty() {
                return Err(anyhow!("{} contains empty entry", label));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_dns_default() {
        let doc: Value = serde_yaml::from_str("port: 7890\n").expect("yaml");
        let config = extract_dns_config(&doc).expect("dns config");
        assert!(config.enable.is_none());
    }

    #[test]
    fn test_apply_patch_and_validate() {
        let doc: Value = serde_yaml::from_str("port: 7890\n").expect("yaml");
        let mut config = extract_dns_config(&doc).expect("dns config");
        let patch = DnsConfigPatch {
            nameserver: Some(vec!["https://dns.google/dns-query".to_string()]),
            enhanced_mode: Some("fake-ip".to_string()),
            ..DnsConfigPatch::default()
        };
        config.apply_patch(patch);
        validate_dns_config(&config).expect("valid dns config");
    }

    #[test]
    fn test_apply_dns_config_removes_when_empty() {
        let mut doc: Value = serde_yaml::from_str("port: 7890\n").expect("yaml");
        let config = DnsConfig::default();
        apply_dns_config(&mut doc, &config).expect("apply dns");
        let map = doc.as_mapping().expect("mapping");
        assert!(map.get(&Value::String("dns".to_string())).is_none());
    }

    #[test]
    fn test_apply_dns_config_writes_mapping() {
        let mut doc: Value = serde_yaml::from_str("port: 7890\n").expect("yaml");
        let config = DnsConfig {
            nameserver: Some(vec!["1.1.1.1".to_string()]),
            ..DnsConfig::default()
        };
        apply_dns_config(&mut doc, &config).expect("apply dns");
        let map = doc.as_mapping().expect("mapping");
        assert!(map.get(&Value::String("dns".to_string())).is_some());
    }

    #[test]
    fn test_validate_rejects_empty_nameserver() {
        let config = DnsConfig {
            nameserver: Some(vec!["".to_string()]),
            ..DnsConfig::default()
        };
        assert!(validate_dns_config(&config).is_err());
    }
}
