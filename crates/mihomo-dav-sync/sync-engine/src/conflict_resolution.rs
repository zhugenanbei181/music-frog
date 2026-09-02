use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MergeStrategy {
    PreferLocal,
    PreferRemote,
    ThreeWayMerge,
    KeepBothWithRename,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfigDiffSummary {
    pub added_keys: Vec<String>,
    pub removed_keys: Vec<String>,
    pub modified_keys: Vec<(String, String, String)>, // (key, local_val, remote_val)
    pub has_conflict: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConflictResolutionResult {
    pub merged_content: String,
    pub strategy_applied: MergeStrategy,
    pub was_clean: bool,
    pub conflicted_keys: Vec<String>,
}

pub fn diff_yaml_configs(local_yaml: &str, remote_yaml: &str) -> anyhow::Result<ConfigDiffSummary> {
    let local: Value = serde_yaml::from_str(local_yaml)?;
    let remote: Value = serde_yaml::from_str(remote_yaml)?;

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

    for (k, v_remote) in remote_map {
        let key_str = k.as_str().unwrap_or("").to_string();
        if let Some(v_local) = local_map.get(k) {
            if v_local != v_remote {
                let local_str = serde_yaml::to_string(v_local)?.trim().to_string();
                let remote_str = serde_yaml::to_string(v_remote)?.trim().to_string();
                modified_keys.push((key_str, local_str, remote_str));
                has_conflict = true;
            }
        } else {
            added_keys.push(key_str);
        }
    }

    for (k, _) in local_map {
        let key_str = k.as_str().unwrap_or("").to_string();
        if !remote_map.contains_key(k) {
            removed_keys.push(key_str);
        }
    }

    Ok(ConfigDiffSummary {
        added_keys,
        removed_keys,
        modified_keys,
        has_conflict,
    })
}

pub fn resolve_config_conflict(
    base_yaml: &str,
    local_yaml: &str,
    remote_yaml: &str,
    strategy: MergeStrategy,
) -> anyhow::Result<ConflictResolutionResult> {
    let base: Value =
        serde_yaml::from_str(base_yaml).unwrap_or(Value::Mapping(serde_yaml::Mapping::new()));
    let local: Value = serde_yaml::from_str(local_yaml)?;
    let remote: Value = serde_yaml::from_str(remote_yaml)?;

    let base_map = base.as_mapping().cloned().unwrap_or_default();
    let local_map = local.as_mapping().cloned().unwrap_or_default();
    let remote_map = remote.as_mapping().cloned().unwrap_or_default();

    let mut merged_map = serde_yaml::Mapping::new();
    let mut conflicted_keys = Vec::new();
    let mut was_clean = true;

    let mut all_keys = HashSet::new();
    for k in base_map
        .keys()
        .chain(local_map.keys())
        .chain(remote_map.keys())
    {
        all_keys.insert(k.clone());
    }

    // Sort keys for deterministic output
    let mut sorted_keys: Vec<_> = all_keys.into_iter().collect();
    sorted_keys.sort_by(|a, b| {
        let a_str = a.as_str().unwrap_or("");
        let b_str = b.as_str().unwrap_or("");
        a_str.cmp(b_str)
    });

    for k in sorted_keys {
        let base_val = base_map.get(&k);
        let local_val = local_map.get(&k);
        let remote_val = remote_map.get(&k);

        let key_str = k.as_str().unwrap_or("").to_string();

        let resolved_val = match (base_val, local_val, remote_val) {
            (_, Some(l), Some(r)) if l == r => Some(l.clone()),
            (Some(b), Some(l), Some(r)) if b == l => Some(r.clone()),
            (Some(b), Some(l), Some(r)) if b == r => Some(l.clone()),
            (None, Some(l), None) => Some(l.clone()),
            (None, None, Some(r)) => Some(r.clone()),
            (Some(b), None, Some(r)) if b == r => None,
            (Some(b), Some(l), None) if b == l => None,
            (Some(_), None, None) => None,
            (_, l, r) => {
                // Conflict
                was_clean = false;
                conflicted_keys.push(key_str.clone());
                match strategy {
                    MergeStrategy::PreferLocal => l.cloned(),
                    MergeStrategy::PreferRemote => r.cloned(),
                    MergeStrategy::ThreeWayMerge | MergeStrategy::KeepBothWithRename => l.cloned(),
                }
            }
        };

        if let Some(val) = resolved_val {
            merged_map.insert(k, val);
        }
    }

    let merged_content = serde_yaml::to_string(&merged_map)?;

    Ok(ConflictResolutionResult {
        merged_content,
        strategy_applied: strategy,
        was_clean,
        conflicted_keys,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diff_yaml_configs() {
        let local = "port: 7890\nsocks-port: 7891\n";
        let remote = "port: 7890\nallow-lan: true\n";

        let diff = diff_yaml_configs(local, remote).unwrap();
        assert_eq!(diff.added_keys, vec!["allow-lan"]);
        assert_eq!(diff.removed_keys, vec!["socks-port"]);
        assert!(diff.modified_keys.is_empty());
        assert!(!diff.has_conflict);

        let local_mod = "port: 7890\n";
        let remote_mod = "port: 8080\n";
        let diff_mod = diff_yaml_configs(local_mod, remote_mod).unwrap();
        assert!(diff_mod.has_conflict);
        assert_eq!(diff_mod.modified_keys[0].0, "port");
    }

    #[test]
    fn test_resolve_config_conflict_clean() {
        let base = "port: 7890\nallow-lan: false\n";
        let local = "port: 7890\nallow-lan: true\n";
        let remote = "port: 8080\nallow-lan: false\n";

        let res =
            resolve_config_conflict(base, local, remote, MergeStrategy::ThreeWayMerge).unwrap();
        assert!(res.was_clean);
        assert!(res.merged_content.contains("port: 8080"));
        assert!(res.merged_content.contains("allow-lan: true"));
    }

    #[test]
    fn test_resolve_config_conflict_conflict() {
        let base = "port: 7890\n";
        let local = "port: 8080\n";
        let remote = "port: 9090\n";

        let res_local =
            resolve_config_conflict(base, local, remote, MergeStrategy::PreferLocal).unwrap();
        assert!(!res_local.was_clean);
        assert_eq!(res_local.conflicted_keys, vec!["port"]);
        assert!(res_local.merged_content.contains("port: 8080"));

        let res_remote =
            resolve_config_conflict(base, local, remote, MergeStrategy::PreferRemote).unwrap();
        assert!(!res_remote.was_clean);
        assert!(res_remote.merged_content.contains("port: 9090"));
    }
}
