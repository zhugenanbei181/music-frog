use serde::{Deserialize, Serialize};
use serde_yaml_ng::{Mapping, Value};

/// Mixin configuration schema for overriding or extending profile settings.
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

/// Rule modifications applied by mixin (prepend, append, replace, delete, overrides).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct RuleMixin {
    #[serde(default)]
    pub prepend: Vec<String>,
    #[serde(default)]
    pub append: Vec<String>,
    #[serde(default)]
    pub replace: Vec<String>,
    #[serde(default)]
    pub delete: Vec<String>,
    #[serde(default)]
    pub overrides: Vec<RuleOverride>,
}

/// Target override rule when matching a given pattern.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct RuleOverride {
    #[serde(alias = "match_pattern", alias = "match")]
    pub pattern: String,
    pub target: String,
}

/// Cascade Overlay Pipeline: merges Base Profile -> Subscription -> Custom Overwrites -> Pre-Mixin -> Post-Mixin.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CascadeOverlayPipeline {
    pub base_profile: Option<String>,
    pub subscription: Option<String>,
    pub custom_overwrites: Option<String>,
    pub pre_mixin: Option<MixinConfig>,
    pub post_mixin: Option<MixinConfig>,
    pub dedup_rules: bool,
    pub dedup_named_sequences: bool,
}

impl CascadeOverlayPipeline {
    pub fn new() -> Self {
        Self {
            base_profile: None,
            subscription: None,
            custom_overwrites: None,
            pre_mixin: None,
            post_mixin: None,
            dedup_rules: true,
            dedup_named_sequences: true,
        }
    }

    pub fn with_base_profile(mut self, yaml: impl Into<String>) -> Self {
        self.base_profile = Some(yaml.into());
        self
    }

    pub fn with_subscription(mut self, yaml: impl Into<String>) -> Self {
        self.subscription = Some(yaml.into());
        self
    }

    pub fn with_custom_overwrites(mut self, yaml: impl Into<String>) -> Self {
        self.custom_overwrites = Some(yaml.into());
        self
    }

    pub fn with_pre_mixin(mut self, mixin: MixinConfig) -> Self {
        self.pre_mixin = Some(mixin);
        self
    }

    pub fn with_post_mixin(mut self, mixin: MixinConfig) -> Self {
        self.post_mixin = Some(mixin);
        self
    }

    pub fn with_dedup_rules(mut self, dedup: bool) -> Self {
        self.dedup_rules = dedup;
        self
    }

    pub fn with_dedup_named_sequences(mut self, dedup: bool) -> Self {
        self.dedup_named_sequences = dedup;
        self
    }

    /// Execute the full cascade overlay pipeline and return the final merged YAML string.
    pub fn execute(&self) -> anyhow::Result<String> {
        let merged_val = self.execute_to_value()?;
        Ok(serde_yaml_ng::to_string(&merged_val)?)
    }

    /// Execute the pipeline to intermediate YAML AST Value.
    pub fn execute_to_value(&self) -> anyhow::Result<Value> {
        // Stage 1: Base Profile
        let mut doc: Value = if let Some(ref base) = self.base_profile {
            if base.trim().is_empty() {
                Value::Mapping(Mapping::new())
            } else {
                serde_yaml_ng::from_str(base).unwrap_or_else(|_| Value::Mapping(Mapping::new()))
            }
        } else {
            Value::Mapping(Mapping::new())
        };

        // Stage 2: Subscription Overlay
        if let Some(ref sub) = self.subscription
            && !sub.trim().is_empty()
            && let Ok(sub_val) = serde_yaml_ng::from_str::<Value>(sub)
        {
            deep_merge_cascade(&mut doc, sub_val, self.dedup_named_sequences);
        }

        // Stage 3: Custom Overwrites
        if let Some(ref custom) = self.custom_overwrites
            && !custom.trim().is_empty()
            && let Ok(custom_val) = serde_yaml_ng::from_str::<Value>(custom)
        {
            deep_merge_cascade(&mut doc, custom_val, self.dedup_named_sequences);
        }

        // Stage 4: Pre-Mixin Overlay
        if let Some(ref pre) = self.pre_mixin {
            apply_mixin_config_to_value(&mut doc, pre, self.dedup_rules, self.dedup_named_sequences)?;
        }

        // Stage 5: Post-Mixin Overlay
        if let Some(ref post) = self.post_mixin {
            apply_mixin_config_to_value(&mut doc, post, self.dedup_rules, self.dedup_named_sequences)?;
        }

        Ok(doc)
    }
}

/// Convenience function executing the 5-stage cascade overlay pipeline.
pub fn cascade_overlay_merge(
    base_profile: Option<&str>,
    subscription: Option<&str>,
    custom_overwrites: Option<&str>,
    pre_mixin: Option<&MixinConfig>,
    post_mixin: Option<&MixinConfig>,
) -> anyhow::Result<String> {
    let mut pipeline = CascadeOverlayPipeline::new();
    if let Some(b) = base_profile {
        pipeline = pipeline.with_base_profile(b);
    }
    if let Some(s) = subscription {
        pipeline = pipeline.with_subscription(s);
    }
    if let Some(c) = custom_overwrites {
        pipeline = pipeline.with_custom_overwrites(c);
    }
    if let Some(pre) = pre_mixin {
        pipeline = pipeline.with_pre_mixin(pre.clone());
    }
    if let Some(post) = post_mixin {
        pipeline = pipeline.with_post_mixin(post.clone());
    }
    pipeline.execute()
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

    apply_mixin_config_to_value(&mut base_val, config, false, false)?;
    let out = serde_yaml_ng::to_string(&base_val)?;
    Ok(out)
}

fn apply_mixin_config_to_value(
    base_val: &mut Value,
    config: &MixinConfig,
    dedup_rules: bool,
    dedup_named: bool,
) -> anyhow::Result<()> {
    let mut mixin_val = serde_yaml_ng::to_value(config)?;

    if let Value::Mapping(ref mut m) = mixin_val {
        m.remove(Value::String("rules".into()));
        m.remove(Value::String("custom-yaml".into()));
    }

    if dedup_named {
        deep_merge_cascade(base_val, mixin_val, true);
    } else {
        deep_merge(base_val, mixin_val);
    }

    if let Some(rules) = &config.rules {
        merge_rules(base_val, rules, dedup_rules);
    }

    if let Some(custom_yaml) = &config.custom_yaml
        && let Ok(custom_val) = serde_yaml_ng::from_str::<Value>(custom_yaml)
    {
        if dedup_named {
            deep_merge_cascade(base_val, custom_val, true);
        } else {
            deep_merge(base_val, custom_val);
        }
    }

    Ok(())
}

/// Recursively deep-merge YAML mappings and scalar fields.
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

/// Advanced deep-merge supporting sequence merging with named object deduplication.
pub fn deep_merge_cascade(base: &mut Value, mixin: Value, dedup_named: bool) {
    match (base, mixin) {
        (Value::Mapping(base_map), Value::Mapping(mixin_map)) => {
            for (k, v) in mixin_map {
                if let Some(base_v) = base_map.get_mut(&k) {
                    if dedup_named && base_v.is_sequence() && v.is_sequence() {
                        if let (Some(base_seq), Value::Sequence(v_seq)) = (base_v.as_sequence_mut(), v) {
                            merge_named_sequences(base_seq, v_seq);
                        }
                    } else {
                        deep_merge_cascade(base_v, v, dedup_named);
                    }
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

/// Merge two sequences where items with a `"name"` field are updated or deduplicated.
pub fn merge_named_sequences(base: &mut Vec<Value>, incoming: Vec<Value>) {
    for item in incoming {
        if let Some(name) = extract_item_name(&item) {
            let mut found = false;
            for base_item in base.iter_mut() {
                if extract_item_name(base_item).as_deref() == Some(&name) {
                    deep_merge(base_item, item.clone());
                    found = true;
                    break;
                }
            }
            if !found {
                base.push(item);
            }
        } else if !base.contains(&item) {
            base.push(item);
        }
    }
}

fn extract_item_name(val: &Value) -> Option<String> {
    val.as_mapping()?
        .get(Value::String("name".to_string()))?
        .as_str()
        .map(str::to_string)
}

fn merge_rules(base: &mut Value, rule_mixin: &RuleMixin, dedup: bool) {
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

    let base_strings: Vec<String> = base_rules
        .into_iter()
        .filter_map(|v| match v {
            Value::String(s) => Some(s),
            _ => None,
        })
        .collect();

    let mut result_strings: Vec<String> = if !rule_mixin.replace.is_empty() {
        rule_mixin.replace.clone()
    } else {
        let mut seq = rule_mixin.prepend.clone();
        seq.extend(base_strings);
        seq.extend(rule_mixin.append.clone());
        seq
    };

    // Apply rule deletion
    if !rule_mixin.delete.is_empty() {
        result_strings.retain(|rule| {
            !rule_mixin.delete.iter().any(|del| {
                let d = del.trim();
                rule.trim() == d || rule.starts_with(&format!("{d},"))
            })
        });
    }

    // Apply rule overrides
    if !rule_mixin.overrides.is_empty() {
        for rule in &mut result_strings {
            for ov in &rule_mixin.overrides {
                let pat = ov.pattern.trim();
                if !pat.is_empty() && (rule.contains(pat) || rule.starts_with(pat)) {
                    *rule = ov.target.clone();
                }
            }
        }
    }

    // Deduplication
    if dedup {
        let mut seen = std::collections::HashSet::new();
        result_strings.retain(|r| seen.insert(r.clone()));
    }

    let new_rules: Vec<Value> = result_strings.into_iter().map(Value::String).collect();
    if let Some(m) = base.as_mapping_mut() {
        m.insert(Value::String("rules".into()), Value::Sequence(new_rules));
    }
}

/// Compose a merged document from user key-level picks over a
/// local-vs-remote diff. `take_remote` names top-level keys whose value is
/// replaced wholesale by the remote one; `accept_removals` names keys that
/// only exist locally and should be dropped.
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
    fn test_rule_delete_and_overrides() {
        let base = "
rules:
  - DOMAIN,ads.com,REJECT
  - DOMAIN,google.com,DIRECT
  - MATCH,DIRECT
";
        let config = MixinConfig {
            rules: Some(RuleMixin {
                delete: vec!["DOMAIN,ads.com".to_string()],
                overrides: vec![RuleOverride {
                    pattern: "google.com".to_string(),
                    target: "DOMAIN,google.com,PROXY".to_string(),
                }],
                ..Default::default()
            }),
            ..Default::default()
        };

        let out = merge_profile_with_config(base, &config).unwrap();
        let out_val: Value = serde_yaml_ng::from_str(&out).unwrap();
        let rules = out_val.get("rules").unwrap().as_sequence().unwrap();

        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].as_str().unwrap(), "DOMAIN,google.com,PROXY");
        assert_eq!(rules[1].as_str().unwrap(), "MATCH,DIRECT");
    }

    #[test]
    fn test_cascade_overlay_pipeline_5_stages() {
        let base = "
mode: rule
port: 7890
rules:
  - MATCH,DIRECT
";
        let sub = "
port: 8080
proxies:
  - name: node-1
    type: ss
    server: 1.1.1.1
    port: 8388
";
        let custom = "
ipv6: true
proxies:
  - name: node-1
    port: 9000
  - name: node-2
    type: trojan
    server: 2.2.2.2
    port: 443
";
        let pre_mixin = MixinConfig {
            mode: Some("script".to_string()),
            ..Default::default()
        };

        let post_mixin = MixinConfig {
            mode: Some("global".to_string()),
            rules: Some(RuleMixin {
                prepend: vec!["DOMAIN,special.com,PROXY".to_string()],
                ..Default::default()
            }),
            ..Default::default()
        };

        let pipeline = CascadeOverlayPipeline::new()
            .with_base_profile(base)
            .with_subscription(sub)
            .with_custom_overwrites(custom)
            .with_pre_mixin(pre_mixin)
            .with_post_mixin(post_mixin);

        let out = pipeline.execute().unwrap();
        let out_val: Value = serde_yaml_ng::from_str(&out).unwrap();

        assert_eq!(out_val.get("mode").unwrap().as_str().unwrap(), "global");
        assert_eq!(out_val.get("port").unwrap().as_u64().unwrap(), 8080);
        assert!(out_val.get("ipv6").unwrap().as_bool().unwrap());

        let proxies = out_val.get("proxies").unwrap().as_sequence().unwrap();
        assert_eq!(proxies.len(), 2);
        assert_eq!(proxies[0].get("name").unwrap().as_str().unwrap(), "node-1");
        assert_eq!(proxies[0].get("port").unwrap().as_u64().unwrap(), 9000);
        assert_eq!(proxies[1].get("name").unwrap().as_str().unwrap(), "node-2");

        let rules = out_val.get("rules").unwrap().as_sequence().unwrap();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].as_str().unwrap(), "DOMAIN,special.com,PROXY");
        assert_eq!(rules[1].as_str().unwrap(), "MATCH,DIRECT");
    }

    #[test]
    fn test_cascade_overlay_merge_function() {
        let base = "port: 7890\nmode: rule\n";
        let sub = "port: 7891\n";
        let pre = MixinConfig {
            log_level: Some("debug".to_string()),
            ..Default::default()
        };
        let post = MixinConfig {
            mode: Some("direct".to_string()),
            ..Default::default()
        };

        let out = cascade_overlay_merge(Some(base), Some(sub), None, Some(&pre), Some(&post)).unwrap();
        let out_val: Value = serde_yaml_ng::from_str(&out).unwrap();
        assert_eq!(out_val.get("port").unwrap().as_u64().unwrap(), 7891);
        assert_eq!(out_val.get("log-level").unwrap().as_str().unwrap(), "debug");
        assert_eq!(out_val.get("mode").unwrap().as_str().unwrap(), "direct");
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
