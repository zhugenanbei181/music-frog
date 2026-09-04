//! YAML AST helper implementations used by script directives.

use regex::Regex;
use serde_yaml_ng::Value;
use std::collections::HashSet;

use super::{COUNTRY_GROUP_DEFS, ScriptError};

// --- YAML AST Helpers ---

/// Add or update a proxy group in the YAML configuration AST.
pub(super) fn add_proxy_group(
    yaml: &mut Value,
    name: &str,
    group_type: &str,
    proxies: &[String],
    url: Option<&str>,
    interval: Option<u64>,
) -> Result<(), ScriptError> {
    let mapping = yaml
        .as_mapping_mut()
        .ok_or_else(|| ScriptError::Runtime("Root YAML is not a mapping".to_string()))?;
    let pg_key = Value::String("proxy-groups".to_string());
    if !mapping.contains_key(&pg_key) || !mapping[&pg_key].is_sequence() {
        mapping.insert(pg_key.clone(), Value::Sequence(Vec::new()));
    }
    let groups_seq = mapping
        .get_mut(&pg_key)
        .and_then(|v| v.as_sequence_mut())
        .ok_or_else(|| ScriptError::Runtime("Failed proxy-groups".to_string()))?;
    let name_key = Value::String("name".to_string());
    let existing_idx = groups_seq.iter().position(|item| {
        item.as_mapping()
            .and_then(|m| m.get(&name_key))
            .and_then(|v| v.as_str())
            == Some(name)
    });

    let mut group_map = serde_yaml_ng::Mapping::new();
    group_map.insert(name_key, Value::String(name.to_string()));
    group_map.insert(
        Value::String("type".to_string()),
        Value::String(group_type.to_string()),
    );
    let proxy_vals: Vec<Value> = proxies
        .iter()
        .map(|p| Value::String(p.to_string()))
        .collect();
    group_map.insert(
        Value::String("proxies".to_string()),
        Value::Sequence(proxy_vals),
    );
    if let Some(u) = url {
        group_map.insert(
            Value::String("url".to_string()),
            Value::String(u.to_string()),
        );
    }
    if let Some(iv) = interval {
        group_map.insert(
            Value::String("interval".to_string()),
            Value::Number(iv.into()),
        );
    }

    if let Some(idx) = existing_idx {
        groups_seq[idx] = Value::Mapping(group_map);
    } else {
        groups_seq.push(Value::Mapping(group_map));
    }
    Ok(())
}

/// Remove rules matching a regular expression or substring from the rules sequence.
pub(super) fn remove_rules(yaml: &mut Value, pattern: &str) -> Result<usize, ScriptError> {
    let re = Regex::new(pattern)
        .map_err(|e| ScriptError::Syntax(format!("Invalid regex pattern: {e}")))?;
    let mapping = yaml
        .as_mapping_mut()
        .ok_or_else(|| ScriptError::Runtime("Root YAML is not a mapping".to_string()))?;
    let rules_key = Value::String("rules".to_string());
    if let Some(rules_seq) = mapping
        .get_mut(&rules_key)
        .and_then(|v| v.as_sequence_mut())
    {
        let before = rules_seq.len();
        rules_seq.retain(|item| item.as_str().is_none_or(|s| !re.is_match(s)));
        Ok(before - rules_seq.len())
    } else {
        Ok(0)
    }
}

/// Filter proxy nodes matching regex. If `invert` is true, remove matching nodes.
pub(super) fn filter_nodes_by_regex(
    yaml: &mut Value,
    pattern: &str,
    invert: bool,
) -> Result<usize, ScriptError> {
    let re = Regex::new(pattern)
        .map_err(|e| ScriptError::Syntax(format!("Invalid regex pattern: {e}")))?;
    let mapping = yaml
        .as_mapping_mut()
        .ok_or_else(|| ScriptError::Runtime("Root YAML is not a mapping".to_string()))?;
    let proxies_key = Value::String("proxies".to_string());
    let name_key = Value::String("name".to_string());
    let mut removed_names = HashSet::new();
    let removed_count;

    if let Some(proxies_seq) = mapping
        .get_mut(&proxies_key)
        .and_then(|v| v.as_sequence_mut())
    {
        let before = proxies_seq.len();
        proxies_seq.retain(|item| {
            let name = item
                .as_mapping()
                .and_then(|m| m.get(&name_key))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let keep = if invert {
                !re.is_match(name)
            } else {
                re.is_match(name)
            };
            if !keep && !name.is_empty() {
                removed_names.insert(name.to_string());
            }
            keep
        });
        removed_count = before - proxies_seq.len();
    } else {
        return Ok(0);
    }

    if !removed_names.is_empty() {
        let pg_key = Value::String("proxy-groups".to_string());
        if let Some(groups_seq) = mapping.get_mut(&pg_key).and_then(|v| v.as_sequence_mut()) {
            for group in groups_seq.iter_mut() {
                if let Some(g_map) = group.as_mapping_mut() {
                    let p_key = Value::String("proxies".to_string());
                    if let Some(p_seq) = g_map.get_mut(&p_key).and_then(|v| v.as_sequence_mut()) {
                        p_seq.retain(|val| {
                            val.as_str()
                                .is_none_or(|p_name| !removed_names.contains(p_name))
                        });
                    }
                }
            }
        }
    }
    Ok(removed_count)
}

/// Set DNS mode and enable state in YAML AST.
pub(super) fn set_dns_mode(
    yaml: &mut Value,
    enhanced_mode: &str,
    enable: bool,
) -> Result<(), ScriptError> {
    let mapping = yaml
        .as_mapping_mut()
        .ok_or_else(|| ScriptError::Runtime("Root YAML is not a mapping".to_string()))?;
    let dns_key = Value::String("dns".to_string());
    if !mapping.contains_key(&dns_key) || !mapping[&dns_key].is_mapping() {
        mapping.insert(
            dns_key.clone(),
            Value::Mapping(serde_yaml_ng::Mapping::new()),
        );
    }
    let dns_map = mapping
        .get_mut(&dns_key)
        .and_then(|v| v.as_mapping_mut())
        .ok_or_else(|| ScriptError::Runtime("Failed dns mapping".to_string()))?;
    dns_map.insert(Value::String("enable".to_string()), Value::Bool(enable));
    dns_map.insert(
        Value::String("enhanced-mode".to_string()),
        Value::String(enhanced_mode.to_string()),
    );
    Ok(())
}

fn extract_proxy_names(yaml: &Value) -> Vec<String> {
    let mut names = Vec::new();
    if let Some(mapping) = yaml.as_mapping() {
        let key = Value::String("proxies".to_string());
        let name_key = Value::String("name".to_string());
        if let Some(proxies_seq) = mapping.get(&key).and_then(|v| v.as_sequence()) {
            for item in proxies_seq {
                if let Some(name) = item
                    .as_mapping()
                    .and_then(|m| m.get(&name_key))
                    .and_then(|v| v.as_str())
                {
                    names.push(name.to_string());
                }
            }
        }
    }
    names
}

/// Dynamically generate country-specific policy groups from the node list.
pub(super) fn generate_country_proxy_groups(
    yaml: &mut Value,
    create_auto_select: bool,
) -> Result<Vec<String>, ScriptError> {
    let proxy_names = extract_proxy_names(yaml);
    let mut generated_groups = Vec::new();

    for def in COUNTRY_GROUP_DEFS {
        let matching: Vec<String> = proxy_names
            .iter()
            .filter(|name| {
                let lower = name.to_lowercase();
                def.match_keywords
                    .iter()
                    .any(|kw| name.contains(kw) || lower.contains(&kw.to_lowercase()))
            })
            .cloned()
            .collect();

        if !matching.is_empty() {
            let group_name = format!("{} {}", def.flag, def.name_zh);
            let mut group_proxies = Vec::new();
            if create_auto_select && matching.len() >= 2 {
                let auto_group_name = format!("{} 自动选择", def.flag);
                add_proxy_group(
                    yaml,
                    &auto_group_name,
                    "url-test",
                    &matching,
                    Some("http://www.gstatic.com/generate_204"),
                    Some(300),
                )?;
                group_proxies.push(auto_group_name);
            }
            group_proxies.extend(matching);
            add_proxy_group(yaml, &group_name, "select", &group_proxies, None, None)?;
            generated_groups.push(group_name);
        }
    }

    if let (false, Some(mapping)) = (generated_groups.is_empty(), yaml.as_mapping_mut()) {
        let pg_key = Value::String("proxy-groups".to_string());
        let name_key = Value::String("name".to_string());
        if let Some(groups_seq) = mapping.get_mut(&pg_key).and_then(|v| v.as_sequence_mut()) {
            for group in groups_seq.iter_mut() {
                if let Some(g_map) = group.as_mapping_mut() {
                    let is_main = g_map
                        .get(&name_key)
                        .and_then(|v| v.as_str())
                        .is_some_and(|n| {
                            n.eq_ignore_ascii_case("PROXY")
                                || n.eq_ignore_ascii_case("PROXIES")
                                || n == "节点选择"
                        });
                    if is_main {
                        let p_key = Value::String("proxies".to_string());
                        if let Some(p_seq) = g_map.get_mut(&p_key).and_then(|v| v.as_sequence_mut())
                        {
                            for g_name in generated_groups.iter().rev() {
                                let val = Value::String(g_name.clone());
                                if !p_seq.contains(&val) {
                                    p_seq.insert(0, val);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(generated_groups)
}

/// Dynamically generate a low-latency auto-test group covering all nodes.
pub(super) fn generate_auto_latency_group(
    yaml: &mut Value,
    group_name: &str,
    url: Option<&str>,
    interval: Option<u64>,
) -> Result<Option<String>, ScriptError> {
    let proxy_names = extract_proxy_names(yaml);
    if proxy_names.is_empty() {
        return Ok(None);
    }
    add_proxy_group(
        yaml,
        group_name,
        "url-test",
        &proxy_names,
        Some(url.unwrap_or("http://www.gstatic.com/generate_204")),
        Some(interval.unwrap_or(300)),
    )?;
    Ok(Some(group_name.to_string()))
}

/// Dynamically generate dedicated streaming service policy groups and routing rules.
pub(super) fn generate_streaming_proxy_groups(
    yaml: &mut Value,
) -> Result<Vec<String>, ScriptError> {
    let proxy_names = extract_proxy_names(yaml);
    let mut candidate_proxies: Vec<String> = COUNTRY_GROUP_DEFS
        .iter()
        .map(|d| format!("{} {}", d.flag, d.name_zh))
        .collect();
    if candidate_proxies.is_empty() || proxy_names.is_empty() {
        candidate_proxies = proxy_names;
    }
    if !candidate_proxies.contains(&"DIRECT".to_string()) {
        candidate_proxies.push("DIRECT".to_string());
    }

    let streaming_services = [
        (
            "🎬 Netflix",
            &[
                "DOMAIN-SUFFIX,netflix.com,🎬 Netflix",
                "DOMAIN-SUFFIX,netflix.net,🎬 Netflix",
                "DOMAIN-SUFFIX,nflximg.net,🎬 Netflix",
                "DOMAIN-SUFFIX,nflxvideo.net,🎬 Netflix",
                "DOMAIN-SUFFIX,nflxext.com,🎬 Netflix",
            ][..],
        ),
        (
            "🐭 Disney+",
            &[
                "DOMAIN-SUFFIX,disneyplus.com,🐭 Disney+",
                "DOMAIN-SUFFIX,disney-plus.net,🐭 Disney+",
                "DOMAIN-SUFFIX,dssott.com,🐭 Disney+",
            ][..],
        ),
        (
            "📹 YouTube",
            &[
                "DOMAIN-SUFFIX,youtube.com,📹 YouTube",
                "DOMAIN-SUFFIX,googlevideo.com,📹 YouTube",
                "DOMAIN-SUFFIX,ytimg.com,📹 YouTube",
            ][..],
        ),
        (
            "🤖 OpenAI",
            &[
                "DOMAIN-SUFFIX,openai.com,🤖 OpenAI",
                "DOMAIN-SUFFIX,chatgpt.com,🤖 OpenAI",
                "DOMAIN-SUFFIX,ai.com,🤖 OpenAI",
            ][..],
        ),
    ];

    let mut created = Vec::new();
    for (group_name, rules) in streaming_services {
        add_proxy_group(yaml, group_name, "select", &candidate_proxies, None, None)?;
        created.push(group_name.to_string());
        if let Some(mapping) = yaml.as_mapping_mut() {
            let r_key = Value::String("rules".to_string());
            if !mapping.contains_key(&r_key) || !mapping[&r_key].is_sequence() {
                mapping.insert(r_key.clone(), Value::Sequence(Vec::new()));
            }
            if let Some(r_seq) = mapping.get_mut(&r_key).and_then(|v| v.as_sequence_mut()) {
                for r in rules.iter().rev() {
                    let val = Value::String((*r).to_string());
                    if !r_seq.contains(&val) {
                        r_seq.insert(0, val);
                    }
                }
            }
        }
    }
    Ok(created)
}

/// Inject direct China LAN and GeoIP routing rules into the configuration.
pub(super) fn generate_china_direct_rules(yaml: &mut Value) -> Result<(), ScriptError> {
    let china_rules = [
        "GEOIP,PRIVATE,DIRECT,no-resolve",
        "GEOIP,CN,DIRECT",
        "DOMAIN-SUFFIX,cn,DIRECT",
        "IP-CIDR,127.0.0.0/8,DIRECT,no-resolve",
        "IP-CIDR,172.16.0.0/12,DIRECT,no-resolve",
        "IP-CIDR,192.168.0.0/16,DIRECT,no-resolve",
        "IP-CIDR,10.0.0.0/8,DIRECT,no-resolve",
    ];
    let mapping = yaml
        .as_mapping_mut()
        .ok_or_else(|| ScriptError::Runtime("Root YAML is not a mapping".to_string()))?;
    let r_key = Value::String("rules".to_string());
    if !mapping.contains_key(&r_key) || !mapping[&r_key].is_sequence() {
        mapping.insert(r_key.clone(), Value::Sequence(Vec::new()));
    }
    if let Some(r_seq) = mapping.get_mut(&r_key).and_then(|v| v.as_sequence_mut()) {
        for r in china_rules.iter().rev() {
            let val = Value::String((*r).to_string());
            if !r_seq.contains(&val) {
                r_seq.insert(0, val);
            }
        }
    }
    Ok(())
}

/// Rename proxy nodes matching regex pattern using replacement string.
pub(super) fn rename_nodes_by_regex(
    yaml: &mut Value,
    pattern: &str,
    replacement: &str,
) -> Result<usize, ScriptError> {
    let re = Regex::new(pattern)
        .map_err(|e| ScriptError::Syntax(format!("Invalid regex pattern: {e}")))?;
    let mapping = yaml
        .as_mapping_mut()
        .ok_or_else(|| ScriptError::Runtime("Root YAML is not a mapping".to_string()))?;
    let proxies_key = Value::String("proxies".to_string());
    let name_key = Value::String("name".to_string());
    let mut renamed_count = 0;
    let mut rename_map = std::collections::HashMap::new();

    if let Some(proxies_seq) = mapping
        .get_mut(&proxies_key)
        .and_then(|v| v.as_sequence_mut())
    {
        for item in proxies_seq.iter_mut() {
            if let Some(p_map) = item.as_mapping_mut()
                && let Some(old_name) = p_map
                    .get(&name_key)
                    .and_then(|v| v.as_str())
                    .map(ToString::to_string)
                    && re.is_match(&old_name) {
                        let new_name = re.replace_all(&old_name, replacement).to_string();
                        if new_name != old_name {
                            p_map.insert(name_key.clone(), Value::String(new_name.clone()));
                            rename_map.insert(old_name, new_name);
                            renamed_count += 1;
                        }
                    }
        }
    }

    if !rename_map.is_empty() {
        let pg_key = Value::String("proxy-groups".to_string());
        if let Some(groups_seq) = mapping.get_mut(&pg_key).and_then(|v| v.as_sequence_mut()) {
            for group in groups_seq.iter_mut() {
                if let Some(g_map) = group.as_mapping_mut() {
                    let p_key = Value::String("proxies".to_string());
                    if let Some(p_seq) = g_map.get_mut(&p_key).and_then(|v| v.as_sequence_mut()) {
                        for p in p_seq.iter_mut() {
                            if let Some(old_name) = p.as_str()
                                && let Some(new_name) = rename_map.get(old_name) {
                                    *p = Value::String(new_name.clone());
                                }
                        }
                    }
                }
            }
        }
    }
    Ok(renamed_count)
}

/// Remove a proxy group by name.
pub(super) fn remove_proxy_group(yaml: &mut Value, name: &str) -> Result<bool, ScriptError> {
    let mapping = yaml
        .as_mapping_mut()
        .ok_or_else(|| ScriptError::Runtime("Root YAML is not a mapping".to_string()))?;
    let pg_key = Value::String("proxy-groups".to_string());
    let name_key = Value::String("name".to_string());
    if let Some(groups_seq) = mapping.get_mut(&pg_key).and_then(|v| v.as_sequence_mut()) {
        let before = groups_seq.len();
        groups_seq.retain(|item| {
            item.as_mapping()
                .and_then(|m| m.get(&name_key))
                .and_then(|v| v.as_str())
                != Some(name)
        });
        Ok(before != groups_seq.len())
    } else {
        Ok(false)
    }
}

/// Prepend a routing rule to the rules list.
pub(super) fn prepend_rule(yaml: &mut Value, rule: &str) -> Result<(), ScriptError> {
    let mapping = yaml
        .as_mapping_mut()
        .ok_or_else(|| ScriptError::Runtime("Root YAML is not a mapping".to_string()))?;
    let r_key = Value::String("rules".to_string());
    if !mapping.contains_key(&r_key) || !mapping[&r_key].is_sequence() {
        mapping.insert(r_key.clone(), Value::Sequence(Vec::new()));
    }
    if let Some(r_seq) = mapping.get_mut(&r_key).and_then(|v| v.as_sequence_mut()) {
        let val = Value::String(rule.to_string());
        if !r_seq.contains(&val) {
            r_seq.insert(0, val);
        }
    }
    Ok(())
}

/// Append a routing rule to the rules list.
pub(super) fn append_rule(yaml: &mut Value, rule: &str) -> Result<(), ScriptError> {
    let mapping = yaml
        .as_mapping_mut()
        .ok_or_else(|| ScriptError::Runtime("Root YAML is not a mapping".to_string()))?;
    let r_key = Value::String("rules".to_string());
    if !mapping.contains_key(&r_key) || !mapping[&r_key].is_sequence() {
        mapping.insert(r_key.clone(), Value::Sequence(Vec::new()));
    }
    if let Some(r_seq) = mapping.get_mut(&r_key).and_then(|v| v.as_sequence_mut()) {
        let val = Value::String(rule.to_string());
        if !r_seq.contains(&val) {
            r_seq.push(val);
        }
    }
    Ok(())
}
