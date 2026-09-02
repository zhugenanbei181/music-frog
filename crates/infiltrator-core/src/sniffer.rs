use crate::settings::app_config_manager;
use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use serde_yaml_ng::{Mapping, Value};
use std::collections::BTreeMap;

/// Sniffer protocol configuration for a single protocol (e.g. HTTP, TLS, QUIC).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub struct ProtocolSniffConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ports: Option<Vec<u16>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub override_destination: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_payload: Option<u32>,
}

impl ProtocolSniffConfig {
    pub fn new(ports: Vec<u16>, override_destination: Option<bool>) -> Self {
        Self {
            ports: Some(ports),
            override_destination,
            max_payload: None,
        }
    }

    pub fn to_json_value(&self) -> Result<serde_json::Value> {
        serde_json::to_value(self).context("serialize protocol sniff config")
    }
}

/// Typed representation of the `sniffer:` section in Mihomo configuration.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct SnifferConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sniff: Option<BTreeMap<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skip_domain: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub force_domain: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_whitelist: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sniffing: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub force_dns_mapping: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parse_pure_ip: Option<bool>,
}

/// Patch structure for updating fields of `SnifferConfig`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct SnifferConfigPatch {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sniff: Option<BTreeMap<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skip_domain: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub force_domain: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_whitelist: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sniffing: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub force_dns_mapping: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parse_pure_ip: Option<bool>,
}

pub type SnifferConfigPayload = SnifferConfigPatch;

impl From<SnifferConfig> for SnifferConfigPatch {
    fn from(c: SnifferConfig) -> Self {
        Self {
            enable: c.enable,
            sniff: c.sniff,
            skip_domain: c.skip_domain,
            force_domain: c.force_domain,
            port_whitelist: c.port_whitelist,
            sniffing: c.sniffing,
            force_dns_mapping: c.force_dns_mapping,
            parse_pure_ip: c.parse_pure_ip,
        }
    }
}

impl From<SnifferConfigPatch> for SnifferConfig {
    fn from(p: SnifferConfigPatch) -> Self {
        Self {
            enable: p.enable,
            sniff: p.sniff,
            skip_domain: p.skip_domain,
            force_domain: p.force_domain,
            port_whitelist: p.port_whitelist,
            sniffing: p.sniffing,
            force_dns_mapping: p.force_dns_mapping,
            parse_pure_ip: p.parse_pure_ip,
        }
    }
}

impl SnifferConfig {
    pub fn apply_patch(&mut self, patch: SnifferConfigPatch) {
        if let Some(v) = patch.enable {
            self.enable = Some(v);
        }
        if let Some(v) = patch.sniff {
            self.sniff = Some(v);
        }
        if let Some(v) = patch.skip_domain {
            self.skip_domain = Some(v);
        }
        if let Some(v) = patch.force_domain {
            self.force_domain = Some(v);
        }
        if let Some(v) = patch.port_whitelist {
            self.port_whitelist = Some(v);
        }
        if let Some(v) = patch.sniffing {
            self.sniffing = Some(v);
        }
        if let Some(v) = patch.force_dns_mapping {
            self.force_dns_mapping = Some(v);
        }
        if let Some(v) = patch.parse_pure_ip {
            self.parse_pure_ip = Some(v);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.enable.is_none()
            && self.sniff.as_ref().is_none_or(|m| m.is_empty())
            && self.skip_domain.as_ref().is_none_or(|v| v.is_empty())
            && self.force_domain.as_ref().is_none_or(|v| v.is_empty())
            && self.port_whitelist.as_ref().is_none_or(|v| v.is_empty())
            && self.sniffing.as_ref().is_none_or(|v| v.is_empty())
            && self.force_dns_mapping.is_none()
            && self.parse_pure_ip.is_none()
    }
}

/// Validate individual port or port range string (e.g. "80", "443", "8000-8080").
pub fn is_valid_port_or_range(s: &str) -> bool {
    let s = s.trim();
    if s.is_empty() {
        return false;
    }
    if let Ok(port) = s.parse::<u16>() {
        return port > 0;
    }
    if let Some((start_s, end_s)) = s.split_once('-')
        && let (Ok(start), Ok(end)) = (start_s.trim().parse::<u16>(), end_s.trim().parse::<u16>()) {
            return start > 0 && start <= end;
    }
    false
}

/// Validate domain pattern string (e.g. "example.com", "+.example.com", "*.google.com").
pub fn is_valid_domain_pattern(s: &str) -> bool {
    let s = s.trim();
    if s.is_empty() {
        return false;
    }
    !s.chars().any(|c| c.is_whitespace() || c.is_control())
}

/// Validate typed `SnifferConfig`.
pub fn validate_typed_sniffer_config(config: &SnifferConfig) -> Result<()> {
    if let Some(ref domains) = config.skip_domain {
        for domain in domains {
            if !is_valid_domain_pattern(domain) {
                bail!("Invalid domain pattern in skip-domain: '{domain}'");
            }
        }
    }

    if let Some(ref domains) = config.force_domain {
        for domain in domains {
            if !is_valid_domain_pattern(domain) {
                bail!("Invalid domain pattern in force-domain: '{domain}'");
            }
        }
    }

    if let Some(ref ports) = config.port_whitelist {
        for port in ports {
            if !is_valid_port_or_range(port) {
                bail!("Invalid port or port range in port-whitelist: '{port}'");
            }
        }
    }

    if let Some(ref sniff_map) = config.sniff {
        for (proto, val) in sniff_map {
            if proto.trim().is_empty() {
                bail!("Sniff protocol name cannot be empty");
            }
            if !val.is_object() {
                bail!("Sniff configuration for protocol '{proto}' must be an object");
            }
        }
    }

    Ok(())
}

/// Validate a generic JSON `Value` representing sniffer configuration.
pub fn validate_sniffer_config(config: &serde_json::Value) -> Result<()> {
    let obj = config
        .as_object()
        .ok_or_else(|| anyhow!("sniffer config must be a JSON object"))?;

    if let Some(enable) = obj.get("enable")
        && !enable.is_boolean()
    {
        bail!("sniffer 'enable' must be a boolean");
    }

    if let Some(val) = obj
        .get("force_dns_mapping")
        .or_else(|| obj.get("force-dns-mapping"))
        && !val.is_boolean()
    {
        bail!("sniffer 'force-dns-mapping' must be a boolean");
    }

    if let Some(val) = obj
        .get("parse_pure_ip")
        .or_else(|| obj.get("parse-pure-ip"))
        && !val.is_boolean()
    {
        bail!("sniffer 'parse-pure-ip' must be a boolean");
    }

    if let Some(skip) = obj.get("skip_domain").or_else(|| obj.get("skip-domain")) {
        let arr = skip
            .as_array()
            .ok_or_else(|| anyhow!("sniffer 'skip-domain' must be an array"))?;
        for item in arr {
            let s = item
                .as_str()
                .ok_or_else(|| anyhow!("'skip-domain' entries must be strings"))?;
            if !is_valid_domain_pattern(s) {
                bail!("Invalid domain in 'skip-domain': '{s}'");
            }
        }
    }

    if let Some(force) = obj.get("force_domain").or_else(|| obj.get("force-domain")) {
        let arr = force
            .as_array()
            .ok_or_else(|| anyhow!("sniffer 'force-domain' must be an array"))?;
        for item in arr {
            let s = item
                .as_str()
                .ok_or_else(|| anyhow!("'force-domain' entries must be strings"))?;
            if !is_valid_domain_pattern(s) {
                bail!("Invalid domain in 'force-domain': '{s}'");
            }
        }
    }

    if let Some(whitelist) = obj
        .get("port_whitelist")
        .or_else(|| obj.get("port-whitelist"))
    {
        let arr = whitelist
            .as_array()
            .ok_or_else(|| anyhow!("sniffer 'port-whitelist' must be an array"))?;
        for item in arr {
            let s = if let Some(str_val) = item.as_str() {
                str_val.to_string()
            } else if let Some(num_val) = item.as_u64() {
                num_val.to_string()
            } else {
                bail!("'port-whitelist' entries must be strings or port numbers");
            };
            if !is_valid_port_or_range(&s) {
                bail!("Invalid port or range in 'port-whitelist': '{s}'");
            }
        }
    }

    if let Some(sniff) = obj.get("sniff") {
        let sniff_obj = sniff
            .as_object()
            .ok_or_else(|| anyhow!("sniffer 'sniff' must be an object"))?;
        for (proto, val) in sniff_obj {
            if proto.trim().is_empty() {
                bail!("Sniff protocol name cannot be empty");
            }
            if !val.is_object() {
                bail!("Sniff configuration for protocol '{proto}' must be an object");
            }
        }
    }

    Ok(())
}

/// Extract typed `SnifferConfig` from a YAML document AST.
pub fn extract_sniffer_config(doc: &Value) -> Result<SnifferConfig> {
    let json_val = extract_sniffer_config_from_doc(doc)?;
    let config: SnifferConfig = serde_json::from_value(json_val).context("parse sniffer config")?;
    validate_typed_sniffer_config(&config)?;
    Ok(config)
}

/// Extract sniffer JSON value from a YAML document AST.
pub fn extract_sniffer_config_from_doc(doc: &Value) -> Result<serde_json::Value> {
    let value = doc
        .get("sniffer")
        .cloned()
        .unwrap_or(Value::Mapping(Mapping::new()));
    let config = serde_json::to_value(value).context("encode sniffer config")?;
    validate_sniffer_config(&config)?;
    Ok(config)
}

/// Apply a generic JSON sniffer configuration into a YAML document AST.
pub fn apply_sniffer_config(doc: &mut Value, config: &serde_json::Value) -> Result<()> {
    validate_sniffer_config(config)?;
    let map = doc
        .as_mapping_mut()
        .ok_or_else(|| anyhow!("profile config is not a mapping"))?;
    let is_empty = config.as_object().is_some_and(|value| value.is_empty());
    if is_empty {
        map.remove(Value::String("sniffer".to_string()));
        return Ok(());
    }

    let yaml_value = serde_yaml_ng::to_value(config).context("decode sniffer config")?;
    map.insert(Value::String("sniffer".to_string()), yaml_value);
    Ok(())
}

/// Apply typed `SnifferConfig` into a YAML document AST.
pub fn apply_typed_sniffer_config(doc: &mut Value, config: &SnifferConfig) -> Result<()> {
    validate_typed_sniffer_config(config)?;
    let map = doc
        .as_mapping_mut()
        .ok_or_else(|| anyhow!("profile config is not a mapping"))?;
    if config.is_empty() {
        map.remove(Value::String("sniffer".to_string()));
        return Ok(());
    }

    let yaml_value = serde_yaml_ng::to_value(config).context("serialize sniffer config to yaml")?;
    map.insert(Value::String("sniffer".to_string()), yaml_value);
    Ok(())
}

/// Apply a `SnifferConfigPatch` into an existing YAML document AST.
pub fn apply_sniffer_patch(doc: &mut Value, patch: &SnifferConfigPatch) -> Result<()> {
    let mut current = extract_sniffer_config(doc).unwrap_or_default();
    current.apply_patch(patch.clone());
    apply_typed_sniffer_config(doc, &current)
}

/// Apply sniffer changes to an in-memory profile YAML string.
pub fn apply_sniffer_to_yaml(content: &str, config: &serde_json::Value) -> Result<String> {
    let mut doc: Value = serde_yaml_ng::from_str(content).context("parse profile yaml")?;
    apply_sniffer_config(&mut doc, config)?;
    serde_yaml_ng::to_string(&doc).context("serialize profile yaml")
}

/// Apply typed `SnifferConfig` to an in-memory profile YAML string.
pub fn apply_typed_sniffer_to_yaml(content: &str, config: &SnifferConfig) -> Result<String> {
    let mut doc: Value = serde_yaml_ng::from_str(content).context("parse profile yaml")?;
    apply_typed_sniffer_config(&mut doc, config)?;
    serde_yaml_ng::to_string(&doc).context("serialize profile yaml")
}

/// Apply a patch to an in-memory profile YAML string.
pub fn apply_sniffer_patch_to_yaml(content: &str, patch: &SnifferConfigPatch) -> Result<String> {
    let mut doc: Value = serde_yaml_ng::from_str(content).context("parse profile yaml")?;
    apply_sniffer_patch(&mut doc, patch)?;
    serde_yaml_ng::to_string(&doc).context("serialize profile yaml")
}

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

    #[test]
    fn test_extract_sniffer_default() {
        let doc: Value = serde_yaml_ng::from_str("port: 7890\n").expect("yaml");
        let config = extract_sniffer_config_from_doc(&doc).expect("extract");
        assert_eq!(config, serde_json::json!({}));

        let typed = extract_sniffer_config(&doc).expect("extract typed");
        assert_eq!(typed, SnifferConfig::default());
        assert!(typed.is_empty());
    }

    #[test]
    fn test_apply_sniffer_empty_removes_key() {
        let mut doc: Value = serde_yaml_ng::from_str(
            r#"
sniffer:
  enable: true
"#,
        )
        .expect("yaml");

        apply_sniffer_config(&mut doc, &serde_json::json!({})).expect("apply");
        let map = doc.as_mapping().expect("mapping");
        assert!(map.get(Value::String("sniffer".to_string())).is_none());
    }

    #[test]
    fn test_apply_sniffer_writes_mapping() {
        let mut doc: Value = serde_yaml_ng::from_str("port: 7890\n").expect("yaml");
        let config = serde_json::json!({
            "enable": true,
            "sniff": {
                "TLS": {
                    "ports": [443, 8443]
                }
            }
        });
        apply_sniffer_config(&mut doc, &config).expect("apply");
        let map = doc.as_mapping().expect("mapping");
        assert!(map.get(Value::String("sniffer".to_string())).is_some());
    }

    #[test]
    fn test_validate_sniffer_rejects_non_object() {
        assert!(validate_sniffer_config(&serde_json::json!([])).is_err());
        assert!(validate_sniffer_config(&serde_json::json!(null)).is_err());
        assert!(validate_sniffer_config(&serde_json::json!("text")).is_err());
    }

    #[test]
    fn test_port_and_range_validation() {
        assert!(is_valid_port_or_range("80"));
        assert!(is_valid_port_or_range("443"));
        assert!(is_valid_port_or_range("8000-8080"));
        assert!(is_valid_port_or_range("1-65535"));

        assert!(!is_valid_port_or_range("0"));
        assert!(!is_valid_port_or_range("65536"));
        assert!(!is_valid_port_or_range("8080-8000"));
        assert!(!is_valid_port_or_range("invalid"));
        assert!(!is_valid_port_or_range(""));
        assert!(!is_valid_port_or_range("80-"));
    }

    #[test]
    fn test_domain_validation() {
        assert!(is_valid_domain_pattern("example.com"));
        assert!(is_valid_domain_pattern("+.example.com"));
        assert!(is_valid_domain_pattern("*.google.com"));

        assert!(!is_valid_domain_pattern(""));
        assert!(!is_valid_domain_pattern("   "));
        assert!(!is_valid_domain_pattern("example .com"));
    }

    #[test]
    fn test_typed_config_patching_and_roundtrip() {
        let mut config = SnifferConfig {
            enable: Some(true),
            skip_domain: Some(vec!["example.com".into()]),
            ..Default::default()
        };

        let patch = SnifferConfigPatch {
            force_domain: Some(vec!["+.google.com".into()]),
            port_whitelist: Some(vec!["80".into(), "443".into(), "8000-8080".into()]),
            force_dns_mapping: Some(true),
            parse_pure_ip: Some(true),
            ..Default::default()
        };

        config.apply_patch(patch);
        assert_eq!(config.enable, Some(true));
        assert_eq!(config.skip_domain, Some(vec!["example.com".into()]));
        assert_eq!(config.force_domain, Some(vec!["+.google.com".into()]));
        assert_eq!(config.force_dns_mapping, Some(true));
        assert_eq!(config.parse_pure_ip, Some(true));

        assert!(validate_typed_sniffer_config(&config).is_ok());

        let yaml = apply_typed_sniffer_to_yaml("port: 7890\n", &config).unwrap();
        let parsed_doc: Value = serde_yaml_ng::from_str(&yaml).unwrap();
        let extracted = extract_sniffer_config(&parsed_doc).unwrap();
        assert_eq!(extracted.enable, Some(true));
        assert_eq!(extracted.force_dns_mapping, Some(true));
    }

    #[test]
    fn test_protocol_sniff_config_helper() {
        let http = ProtocolSniffConfig::new(vec![80, 8080], Some(true));
        let tls = ProtocolSniffConfig::new(vec![443, 8443], None);
        let quic = ProtocolSniffConfig::new(vec![443], Some(true));

        let mut sniff_map = BTreeMap::new();
        sniff_map.insert("HTTP".to_string(), http.to_json_value().unwrap());
        sniff_map.insert("TLS".to_string(), tls.to_json_value().unwrap());
        sniff_map.insert("QUIC".to_string(), quic.to_json_value().unwrap());

        let config = SnifferConfig {
            enable: Some(true),
            sniff: Some(sniff_map),
            ..Default::default()
        };

        assert!(validate_typed_sniffer_config(&config).is_ok());
    }

    /// sniffer 读写必须落在 settings `configs_dir` 重定向后的目录：
    /// 重定向态下读回成功即证明写入落点正确，`<home>/configs` 保持未创建。
    #[tokio::test]
    async fn test_sniffer_io_follows_configs_dir_redirect() {
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
