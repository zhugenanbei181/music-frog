//! Pure YAML conflict inspection for synchronization surfaces.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_yaml_ng::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfigDiffSummary {
    pub added_keys: Vec<String>,
    pub removed_keys: Vec<String>,
    pub modified_keys: Vec<(String, String, String)>,
    pub has_conflict: bool,
}

pub fn diff_yaml_configs(local_yaml: &str, remote_yaml: &str) -> Result<ConfigDiffSummary> {
    let local: Value = serde_yaml_ng::from_str(local_yaml)?;
    let remote: Value = serde_yaml_ng::from_str(remote_yaml)?;

    let local_map = local
        .as_mapping()
        .ok_or_else(|| anyhow::anyhow!("Local YAML is not a mapping"))?;
    let remote_map = remote
        .as_mapping()
        .ok_or_else(|| anyhow::anyhow!("Remote YAML is not a mapping"))?;

    let mut added_keys = Vec::new();
    let mut removed_keys = Vec::new();
    let mut modified_keys = Vec::new();
    let mut has_conflict = false;

    for (key, remote_value) in remote_map {
        let key_string = key.as_str().unwrap_or_default().to_string();
        if let Some(local_value) = local_map.get(key) {
            if local_value != remote_value {
                let local_string = serde_yaml_ng::to_string(local_value)?.trim().to_string();
                let remote_string = serde_yaml_ng::to_string(remote_value)?.trim().to_string();
                modified_keys.push((key_string, local_string, remote_string));
                has_conflict = true;
            }
        } else {
            added_keys.push(key_string);
        }
    }

    for (key, _) in local_map {
        let key_string = key.as_str().unwrap_or_default().to_string();
        if !remote_map.contains_key(key) {
            removed_keys.push(key_string);
        }
    }

    Ok(ConfigDiffSummary {
        added_keys,
        removed_keys,
        modified_keys,
        has_conflict,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_added_removed_and_modified_top_level_keys() {
        let diff = diff_yaml_configs(
            "port: 7890\nsocks-port: 7891\n",
            "port: 8080\nallow-lan: true\n",
        )
        .expect("diff");
        assert_eq!(diff.added_keys, vec!["allow-lan"]);
        assert_eq!(diff.removed_keys, vec!["socks-port"]);
        assert_eq!(diff.modified_keys[0].0, "port");
        assert!(diff.has_conflict);
    }
}
