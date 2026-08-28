use std::collections::BTreeMap;

use anyhow::{Context, Result, anyhow};
use mihomo_config::ConfigManager;
use serde::{Deserialize, Serialize};
use serde_yml::{Mapping, Value};

pub type ProxyProviders = BTreeMap<String, serde_json::Value>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyProvidersPayload {
    pub providers: ProxyProviders,
}

pub async fn load_proxy_providers() -> Result<ProxyProviders> {
    let doc = load_profile_doc().await?;
    extract_proxy_providers_from_doc(&doc)
}

pub async fn save_proxy_providers(providers: ProxyProviders) -> Result<ProxyProviders> {
    let manager = ConfigManager::new().context("init config manager")?;
    let profile = manager
        .get_current()
        .await
        .context("load current profile")?;
    let content = manager
        .load(&profile)
        .await
        .context("read profile config")?;
    let mut doc: Value = serde_yml::from_str(&content).context("parse profile yaml")?;

    apply_proxy_providers(&mut doc, &providers)?;

    let updated = serde_yml::to_string(&doc).context("serialize profile yaml")?;
    manager
        .save(&profile, &updated)
        .await
        .context("save profile config")?;
    Ok(providers)
}

async fn load_profile_doc() -> Result<Value> {
    let manager = ConfigManager::new().context("init config manager")?;
    let profile = manager
        .get_current()
        .await
        .context("load current profile")?;
    let content = manager
        .load(&profile)
        .await
        .context("read profile config")?;
    serde_yml::from_str(&content).context("parse profile yaml")
}

pub fn extract_proxy_providers_from_doc(doc: &Value) -> Result<ProxyProviders> {
    let value = doc
        .get("proxy-providers")
        .cloned()
        .unwrap_or(Value::Mapping(Mapping::new()));
    let mapping = value
        .as_mapping()
        .ok_or_else(|| anyhow!("proxy-providers is not a mapping"))?;
    let mut providers = BTreeMap::new();
    for (key, val) in mapping {
        let name = key
            .as_str()
            .ok_or_else(|| anyhow!("proxy-providers contains non-string key"))?;
        let json_value = serde_json::to_value(val).context("encode proxy provider")?;
        providers.insert(name.to_string(), json_value);
    }
    Ok(providers)
}

fn apply_proxy_providers(doc: &mut Value, providers: &ProxyProviders) -> Result<()> {
    let map = doc
        .as_mapping_mut()
        .ok_or_else(|| anyhow!("profile config is not a mapping"))?;
    if providers.is_empty() {
        map.remove(Value::String("proxy-providers".to_string()));
        return Ok(());
    }

    let mut yaml_map = Mapping::new();
    for (name, value) in providers {
        let yaml_value = serde_yml::to_value(value).context("decode proxy provider")?;
        yaml_map.insert(Value::String(name.to_string()), yaml_value);
    }
    map.insert(
        Value::String("proxy-providers".to_string()),
        Value::Mapping(yaml_map),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_proxy_providers_default() {
        let doc: Value = serde_yml::from_str("port: 7890\n").expect("yaml");
        let providers = extract_proxy_providers_from_doc(&doc).expect("extract");
        assert!(providers.is_empty());
    }

    #[test]
    fn test_extract_proxy_providers_invalid_key() {
        let doc: Value = serde_yml::from_str(
            r#"
proxy-providers:
  1:
    type: http
"#,
        )
        .expect("yaml");
        assert!(extract_proxy_providers_from_doc(&doc).is_err());
    }

    #[test]
    fn test_apply_proxy_providers_empty_removes() {
        let mut doc: Value = serde_yml::from_str(
            r#"
proxy-providers:
  test:
    type: http
"#,
        )
        .expect("yaml");
        apply_proxy_providers(&mut doc, &ProxyProviders::new()).expect("apply");
        let map = doc.as_mapping().expect("mapping");
        assert!(
            map.get(Value::String("proxy-providers".to_string()))
                .is_none()
        );
    }

    #[test]
    fn test_apply_proxy_providers_writes_mapping() {
        let mut doc: Value = serde_yml::from_str("port: 7890\n").expect("yaml");
        let mut providers = ProxyProviders::new();
        providers.insert(
            "test".to_string(),
            serde_json::json!({
                "type": "http",
                "url": "https://example.com/providers.yaml"
            }),
        );

        apply_proxy_providers(&mut doc, &providers).expect("apply");
        let map = doc.as_mapping().expect("mapping");
        assert!(
            map.get(Value::String("proxy-providers".to_string()))
                .is_some()
        );
    }
}
