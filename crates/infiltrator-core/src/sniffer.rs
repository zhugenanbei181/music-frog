use anyhow::{Context, Result, anyhow};
use mihomo_config::ConfigManager;
use serde_yml::{Mapping, Value};

pub async fn load_sniffer_config() -> Result<serde_json::Value> {
    let doc = load_profile_doc().await?;
    extract_sniffer_config_from_doc(&doc)
}

pub async fn save_sniffer_config(config: serde_json::Value) -> Result<serde_json::Value> {
    validate_sniffer_config(&config)?;
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

    apply_sniffer_config(&mut doc, &config)?;

    let updated = serde_yml::to_string(&doc).context("serialize profile yaml")?;
    manager
        .save(&profile, &updated)
        .await
        .context("save profile config")?;
    Ok(config)
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

fn validate_sniffer_config(config: &serde_json::Value) -> Result<()> {
    if !config.is_object() {
        return Err(anyhow!("sniffer config must be a JSON object"));
    }
    Ok(())
}

pub fn extract_sniffer_config_from_doc(doc: &Value) -> Result<serde_json::Value> {
    let value = doc
        .get("sniffer")
        .cloned()
        .unwrap_or(Value::Mapping(Mapping::new()));
    let config = serde_json::to_value(value).context("encode sniffer config")?;
    validate_sniffer_config(&config)?;
    Ok(config)
}

fn apply_sniffer_config(doc: &mut Value, config: &serde_json::Value) -> Result<()> {
    validate_sniffer_config(config)?;
    let map = doc
        .as_mapping_mut()
        .ok_or_else(|| anyhow!("profile config is not a mapping"))?;
    let is_empty = config
        .as_object()
        .map(|value| value.is_empty())
        .unwrap_or(false);
    if is_empty {
        map.remove(Value::String("sniffer".to_string()));
        return Ok(());
    }

    let yaml_value = serde_yml::to_value(config).context("decode sniffer config")?;
    map.insert(Value::String("sniffer".to_string()), yaml_value);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_sniffer_default() {
        let doc: Value = serde_yml::from_str("port: 7890\n").expect("yaml");
        let config = extract_sniffer_config_from_doc(&doc).expect("extract");
        assert_eq!(config, serde_json::json!({}));
    }

    #[test]
    fn test_apply_sniffer_empty_removes_key() {
        let mut doc: Value = serde_yml::from_str(
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
        let mut doc: Value = serde_yml::from_str("port: 7890\n").expect("yaml");
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
}
