use serde::{Deserialize, Serialize};
use serde_yaml_ng::{Mapping, Value};

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct MixinConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_level: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ipv6: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_lan: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mixed_port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_controller: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_ui: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub dns: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tun: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sniffer: Option<Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub rules: Option<RuleMixin>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxies: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxy_groups: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxy_providers: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_providers: Option<Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_yaml: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct RuleMixin {
    #[serde(default)]
    pub prepend: Vec<String>,
    #[serde(default)]
    pub append: Vec<String>,
    #[serde(default)]
    pub replace: Vec<String>,
}

pub fn merge_profile_with_mixin(base_yaml: &str, mixin_yaml: &str) -> anyhow::Result<String> {
    let mixin: MixinConfig = serde_yaml_ng::from_str(mixin_yaml)?;
    merge_profile_with_config(base_yaml, &mixin)
}

pub fn merge_profile_with_config(base_yaml: &str, config: &MixinConfig) -> anyhow::Result<String> {
    let mut base_val: Value = if base_yaml.trim().is_empty() {
        Value::Mapping(Mapping::new())
    } else {
        serde_yaml_ng::from_str(base_yaml).unwrap_or_else(|_| Value::Mapping(Mapping::new()))
    };

    let mut mixin_val = serde_yaml_ng::to_value(config)?;

    if let Value::Mapping(ref mut m) = mixin_val {
        m.remove(Value::String("rules".into()));
        m.remove(Value::String("custom-yaml".into()));
    }

    deep_merge(&mut base_val, mixin_val);

    if let Some(rules) = &config.rules {
        merge_rules(&mut base_val, rules);
    }

    if let Some(custom_yaml) = &config.custom_yaml
        && let Ok(custom_val) = serde_yaml_ng::from_str::<Value>(custom_yaml)
    {
        deep_merge(&mut base_val, custom_val);
    }

    let out = serde_yaml_ng::to_string(&base_val)?;
    Ok(out)
}

pub fn deep_merge(base: &mut Value, mixin: Value) {
    match (base, mixin) {
        (Value::Mapping(base_map), Value::Mapping(mixin_map)) => {
            for (k, v) in mixin_map {
                if let Some(base_v) = base_map.get_mut(&k) {
                    deep_merge(base_v, v);
                } else {
                    base_map.insert(k, v);
                }
            }
        }
        (base_val, mixin_val) => {
            *base_val = mixin_val;
        }
    }
}

fn merge_rules(base: &mut Value, rule_mixin: &RuleMixin) {
    if !base.is_mapping() {
        *base = Value::Mapping(Mapping::new());
    }

    let base_rules = match base.as_mapping_mut() {
        Some(m) => {
            if let Some(Value::Sequence(seq)) = m.get_mut(Value::String("rules".into())) {
                std::mem::take(seq)
            } else {
                Vec::new()
            }
        }
        None => Vec::new(),
    };

    let new_rules = if !rule_mixin.replace.is_empty() {
        rule_mixin
            .replace
            .iter()
            .map(|s| Value::String(s.clone()))
            .collect()
    } else {
        let mut seq: Vec<Value> = rule_mixin
            .prepend
            .iter()
            .map(|s| Value::String(s.clone()))
            .collect();
        seq.extend(base_rules);
        seq.extend(rule_mixin.append.iter().map(|s| Value::String(s.clone())));
        seq
    };

    if let Some(m) = base.as_mapping_mut() {
        m.insert(Value::String("rules".into()), Value::Sequence(new_rules));
    }
}

/// Compose a merged document from user key-level picks over a
/// local-vs-remote diff. `take_remote` names top-level keys whose value is
/// replaced wholesale by the remote one (added or modified keys); `accept_removals`
/// names keys that only exist locally and should be dropped to match the
/// remote document. Anything not picked keeps the local value, so the user
/// stays in control of every top-level difference.
pub fn merge_yaml_key_picks(
    local_yaml: &str,
    remote_yaml: &str,
    take_remote: &[String],
    accept_removals: &[String],
) -> anyhow::Result<String> {
    let mut local: Value = serde_yaml_ng::from_str(local_yaml)
        .map_err(|error| anyhow::anyhow!("本地 YAML 解析失败: {error}"))?;
    let remote: Value = serde_yaml_ng::from_str(remote_yaml)
        .map_err(|error| anyhow::anyhow!("远端 YAML 解析失败: {error}"))?;
    let local_map = local
        .as_mapping_mut()
        .ok_or_else(|| anyhow::anyhow!("本地 YAML 顶层不是映射"))?;
    let remote_map = remote
        .as_mapping()
        .ok_or_else(|| anyhow::anyhow!("远端 YAML 顶层不是映射"))?;

    for key in accept_removals {
        local_map.remove(Value::String(key.clone()));
    }
    for key in take_remote {
        let Some(value) = remote_map.get(Value::String(key.clone())) else {
            return Err(anyhow::anyhow!("远端配置中不存在键: {key}"));
        };
        local_map.insert(Value::String(key.clone()), value.clone());
    }

    Ok(serde_yaml_ng::to_string(&local)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scalar_override() {
        let base = "mode: rule\nipv6: false\nmixed-port: 7890\n";
        let mixin = "mode: global\nmixed-port: 7891\n";
        let out = merge_profile_with_mixin(base, mixin).unwrap();
        let out_val: Value = serde_yaml_ng::from_str(&out).unwrap();

        assert_eq!(out_val.get("mode").unwrap().as_str().unwrap(), "global");
        assert!(!out_val.get("ipv6").unwrap().as_bool().unwrap());
        assert_eq!(out_val.get("mixed-port").unwrap().as_u64().unwrap(), 7891);
    }

    #[test]
    fn test_deep_merge_tables() {
        let base = "
dns:
  enable: true
  listen: 0.0.0.0:53
  nameserver:
    - 8.8.8.8
tun:
  enable: false
  stack: gvisor
";
        let mixin = "
dns:
  listen: 127.0.0.1:5353
  nameserver:
    - 1.1.1.1
tun:
  enable: true
";
        let out = merge_profile_with_mixin(base, mixin).unwrap();
        let out_val: Value = serde_yaml_ng::from_str(&out).unwrap();

        let dns = out_val.get("dns").unwrap();
        assert!(dns.get("enable").unwrap().as_bool().unwrap());
        assert_eq!(
            dns.get("listen").unwrap().as_str().unwrap(),
            "127.0.0.1:5353"
        );

        let tun = out_val.get("tun").unwrap();
        assert!(tun.get("enable").unwrap().as_bool().unwrap());
        assert_eq!(tun.get("stack").unwrap().as_str().unwrap(), "gvisor");
    }

    #[test]
    fn test_rule_prepend() {
        let base = "
rules:
  - MATCH,DIRECT
";
        let mixin = "
rules:
  prepend:
    - DOMAIN,example.com,PROXY
";
        let out = merge_profile_with_mixin(base, mixin).unwrap();
        let out_val: Value = serde_yaml_ng::from_str(&out).unwrap();

        let rules = out_val.get("rules").unwrap().as_sequence().unwrap();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].as_str().unwrap(), "DOMAIN,example.com,PROXY");
        assert_eq!(rules[1].as_str().unwrap(), "MATCH,DIRECT");
    }

    #[test]
    fn test_rule_replace() {
        let base = "
rules:
  - MATCH,DIRECT
";
        let mixin = "
rules:
  replace:
    - DOMAIN,example.com,PROXY
";
        let out = merge_profile_with_mixin(base, mixin).unwrap();
        let out_val: Value = serde_yaml_ng::from_str(&out).unwrap();

        let rules = out_val.get("rules").unwrap().as_sequence().unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].as_str().unwrap(), "DOMAIN,example.com,PROXY");
    }

    #[test]
    fn test_custom_yaml() {
        let base = "foo: 1";
        let mixin = "
custom-yaml: |
  bar: 2
  foo: 3
";
        let out = merge_profile_with_mixin(base, mixin).unwrap();
        let out_val: Value = serde_yaml_ng::from_str(&out).unwrap();

        assert_eq!(out_val.get("bar").unwrap().as_i64().unwrap(), 2);
        assert_eq!(out_val.get("foo").unwrap().as_i64().unwrap(), 3);
    }

    #[test]
    fn test_key_picks_merge_modified_added_and_removed() {
        let local = "port: 7890\nkeep: local\nremoved-by-remote: 1\nlog-level: info\n";
        let remote = "port: 8080\nkeep: local\nadded-by-remote: true\nlog-level: warning\n";
        let out = merge_yaml_key_picks(
            local,
            remote,
            &["port".to_string(), "added-by-remote".to_string()],
            &["removed-by-remote".to_string()],
        )
        .unwrap();
        let doc: Value = serde_yaml_ng::from_str(&out).unwrap();
        assert_eq!(doc.get("port").unwrap().as_u64().unwrap(), 8080);
        assert_eq!(doc.get("keep").unwrap().as_str().unwrap(), "local");
        assert!(doc.get("added-by-remote").unwrap().as_bool().unwrap());
        assert_eq!(doc.get("log-level").unwrap().as_str().unwrap(), "info");
        assert!(doc.get("removed-by-remote").is_none());
    }

    #[test]
    fn test_key_picks_rejects_unknown_remote_key() {
        let error = merge_yaml_key_picks("a: 1\n", "a: 2\n", &["ghost".to_string()], &[])
            .unwrap_err()
            .to_string();
        assert!(error.contains("ghost"), "error: {error}");
    }
}
