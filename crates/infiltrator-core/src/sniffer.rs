use crate::settings::app_config_manager;
use anyhow::{Context, Result, anyhow};
use serde_yaml_ng::{Mapping, Value};

pub async fn load_sniffer_config() -> Result<serde_json::Value> {
    let doc = load_profile_doc().await?;
    extract_sniffer_config_from_doc(&doc)
}

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

/// Apply sniffer changes to an in-memory profile document for the shared
/// atomic Apply path.
pub fn apply_sniffer_to_yaml(content: &str, config: &serde_json::Value) -> Result<String> {
    let mut doc: Value = serde_yaml_ng::from_str(content).context("parse profile yaml")?;
    apply_sniffer_config(&mut doc, config)?;
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

    let yaml_value = serde_yaml_ng::to_value(config).context("decode sniffer config")?;
    map.insert(Value::String("sniffer".to_string()), yaml_value);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_sniffer_default() {
        let doc: Value = serde_yaml_ng::from_str("port: 7890\n").expect("yaml");
        let config = extract_sniffer_config_from_doc(&doc).expect("extract");
        assert_eq!(config, serde_json::json!({}));
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
