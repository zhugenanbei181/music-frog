use anyhow::{Context, Result};
use regex::Regex;
use serde_yaml_ng::Value;
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum DeduplicationStrategy {
    #[default]
    Disabled,
    KeepFirst,
    KeepLast,
    AppendIndex,
}

#[derive(Debug, Clone)]
pub struct RenameRule {
    pub pattern: Regex,
    pub replacement: String,
}

#[derive(Debug, Clone, Default)]
pub struct FilterRule {
    pub include_keywords: Vec<Regex>,
    pub exclude_keywords: Vec<Regex>,
    pub rename_rules: Vec<RenameRule>,
    pub exclude_types: Vec<String>,
    pub deduplication: DeduplicationStrategy,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FilterReport {
    pub total_input: usize,
    pub passed: usize,
    pub excluded_by_blacklist: usize,
    pub excluded_by_whitelist: usize,
    pub excluded_by_type: usize,
    pub renamed: usize,
    pub deduplicated: usize,
}

#[derive(Debug, Clone, Default)]
pub struct SubscriptionFilterPipeline {
    pub rule: FilterRule,
}

impl SubscriptionFilterPipeline {
    pub fn new(rule: FilterRule) -> Self {
        Self { rule }
    }

    pub fn apply_to_yaml(&self, yaml_str: &str) -> Result<(String, FilterReport)> {
        let mut doc: Value = serde_yaml_ng::from_str(yaml_str).context("Failed to parse YAML")?;
        let mut report = FilterReport::default();

        if let Some(proxies) = doc.get_mut("proxies")
            && let Some(proxies_seq) = proxies.as_sequence_mut()
        {
            report.total_input = proxies_seq.len();

            let mut new_proxies = Vec::new();
            let mut seen_names = HashSet::new();
            let mut last_seen_index = std::collections::HashMap::new();

            for proxy in proxies_seq.iter() {
                let mut proxy = proxy.clone();

                let (name, proxy_type) = if let Some(map) = proxy.as_mapping() {
                    let name = map
                        .get(Value::String("name".to_string()))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let proxy_type = map
                        .get(Value::String("type".to_string()))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    (name, proxy_type)
                } else {
                    continue;
                };

                if self.rule.exclude_types.contains(&proxy_type) {
                    report.excluded_by_type += 1;
                    continue;
                }

                if !self.rule.include_keywords.is_empty() {
                    let mut included = false;
                    for regex in &self.rule.include_keywords {
                        if regex.is_match(&name) {
                            included = true;
                            break;
                        }
                    }
                    if !included {
                        report.excluded_by_whitelist += 1;
                        continue;
                    }
                }

                let mut excluded = false;
                for regex in &self.rule.exclude_keywords {
                    if regex.is_match(&name) {
                        excluded = true;
                        break;
                    }
                }
                if excluded {
                    report.excluded_by_blacklist += 1;
                    continue;
                }

                let mut current_name = name;
                let mut renamed = false;
                for rule in &self.rule.rename_rules {
                    let new_name = rule
                        .pattern
                        .replace_all(&current_name, &rule.replacement)
                        .to_string();
                    if new_name != current_name {
                        current_name = new_name;
                        renamed = true;
                    }
                }
                if renamed {
                    report.renamed += 1;
                }

                match self.rule.deduplication {
                    DeduplicationStrategy::Disabled => {}
                    DeduplicationStrategy::KeepFirst => {
                        if seen_names.contains(&current_name) {
                            report.deduplicated += 1;
                            continue;
                        }
                        seen_names.insert(current_name.clone());
                    }
                    DeduplicationStrategy::KeepLast => {
                        if let Some(idx) = last_seen_index.get(&current_name) {
                            report.deduplicated += 1;
                            new_proxies[*idx] = Value::Null; // mark for removal
                        }
                        last_seen_index.insert(current_name.clone(), new_proxies.len());
                    }
                    DeduplicationStrategy::AppendIndex => {
                        let mut count = 1;
                        let original_name = current_name.clone();
                        while seen_names.contains(&current_name) {
                            current_name = format!("{} ({})", original_name, count);
                            count += 1;
                        }
                        if count > 1 {
                            report.deduplicated += 1;
                        }
                        seen_names.insert(current_name.clone());
                    }
                }

                if let Some(map) = proxy.as_mapping_mut() {
                    map.insert(
                        Value::String("name".to_string()),
                        Value::String(current_name),
                    );
                }

                new_proxies.push(proxy);
            }

            if self.rule.deduplication == DeduplicationStrategy::KeepLast {
                new_proxies.retain(|p| !p.is_null());
            }

            report.passed = new_proxies.len();
            *proxies_seq = new_proxies;
        }

        let out = serde_yaml_ng::to_string(&doc).context("Failed to serialize YAML")?;
        Ok((out, report))
    }

    pub fn filter_proxy_names(&self, names: &[String]) -> (Vec<String>, FilterReport) {
        let mut report = FilterReport {
            total_input: names.len(),
            ..FilterReport::default()
        };

        let mut out_names = Vec::new();
        let mut seen_names = HashSet::new();
        let mut last_seen_index = std::collections::HashMap::new();

        for name in names {
            if !self.rule.include_keywords.is_empty() {
                let mut included = false;
                for regex in &self.rule.include_keywords {
                    if regex.is_match(name) {
                        included = true;
                        break;
                    }
                }
                if !included {
                    report.excluded_by_whitelist += 1;
                    continue;
                }
            }

            let mut excluded = false;
            for regex in &self.rule.exclude_keywords {
                if regex.is_match(name) {
                    excluded = true;
                    break;
                }
            }
            if excluded {
                report.excluded_by_blacklist += 1;
                continue;
            }

            let mut current_name = name.clone();
            let mut renamed = false;
            for rule in &self.rule.rename_rules {
                let new_name = rule
                    .pattern
                    .replace_all(&current_name, &rule.replacement)
                    .to_string();
                if new_name != current_name {
                    current_name = new_name;
                    renamed = true;
                }
            }
            if renamed {
                report.renamed += 1;
            }

            match self.rule.deduplication {
                DeduplicationStrategy::Disabled => {}
                DeduplicationStrategy::KeepFirst => {
                    if seen_names.contains(&current_name) {
                        report.deduplicated += 1;
                        continue;
                    }
                    seen_names.insert(current_name.clone());
                }
                DeduplicationStrategy::KeepLast => {
                    if let Some(idx) = last_seen_index.get(&current_name) {
                        report.deduplicated += 1;
                        out_names[*idx] = String::new(); // mark for removal
                    }
                    last_seen_index.insert(current_name.clone(), out_names.len());
                }
                DeduplicationStrategy::AppendIndex => {
                    let mut count = 1;
                    let original_name = current_name.clone();
                    while seen_names.contains(&current_name) {
                        current_name = format!("{} ({})", original_name, count);
                        count += 1;
                    }
                    if count > 1 {
                        report.deduplicated += 1;
                    }
                    seen_names.insert(current_name.clone());
                }
            }

            out_names.push(current_name);
        }

        if self.rule.deduplication == DeduplicationStrategy::KeepLast {
            out_names.retain(|n| !n.is_empty());
        }

        report.passed = out_names.len();
        (out_names, report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_whitelist() {
        let rule = FilterRule {
            include_keywords: vec![Regex::new("HK").unwrap(), Regex::new("JP").unwrap()],
            ..Default::default()
        };
        let pipeline = SubscriptionFilterPipeline::new(rule);
        let names = vec!["HK-1".to_string(), "US-1".to_string(), "JP-2".to_string()];
        let (res, rep) = pipeline.filter_proxy_names(&names);
        assert_eq!(res, vec!["HK-1", "JP-2"]);
        assert_eq!(rep.passed, 2);
        assert_eq!(rep.excluded_by_whitelist, 1);
    }

    #[test]
    fn test_blacklist() {
        let rule = FilterRule {
            exclude_keywords: vec![Regex::new("剩余流量").unwrap(), Regex::new("官网").unwrap()],
            ..Default::default()
        };
        let pipeline = SubscriptionFilterPipeline::new(rule);
        let names = vec![
            "HK-1".to_string(),
            "剩余流量: 10GB".to_string(),
            "官网".to_string(),
        ];
        let (res, rep) = pipeline.filter_proxy_names(&names);
        assert_eq!(res, vec!["HK-1"]);
        assert_eq!(rep.passed, 1);
        assert_eq!(rep.excluded_by_blacklist, 2);
    }

    #[test]
    fn test_regex_replacement() {
        let rule = FilterRule {
            rename_rules: vec![RenameRule {
                pattern: Regex::new(r"🇭🇰 香港-(\d+)").unwrap(),
                replacement: "HK-$1".to_string(),
            }],
            ..Default::default()
        };
        let pipeline = SubscriptionFilterPipeline::new(rule);
        let names = vec!["🇭🇰 香港-01".to_string(), "🇭🇰 香港-02".to_string()];
        let (res, rep) = pipeline.filter_proxy_names(&names);
        assert_eq!(res, vec!["HK-01", "HK-02"]);
        assert_eq!(rep.passed, 2);
        assert_eq!(rep.renamed, 2);
    }

    #[test]
    fn test_deduplication_append_index() {
        let rule = FilterRule {
            deduplication: DeduplicationStrategy::AppendIndex,
            ..Default::default()
        };
        let pipeline = SubscriptionFilterPipeline::new(rule);
        let names = vec![
            "HK-01".to_string(),
            "HK-01".to_string(),
            "HK-01".to_string(),
        ];
        let (res, rep) = pipeline.filter_proxy_names(&names);
        assert_eq!(res, vec!["HK-01", "HK-01 (1)", "HK-01 (2)"]);
        assert_eq!(rep.deduplicated, 2);
    }

    #[test]
    fn test_yaml_transformation() {
        let yaml = r#"
proxies:
  - name: "🇭🇰 香港-01"
    type: ss
  - name: "剩余流量: 100G"
    type: ss
  - name: "JP-01"
    type: trojan
  - name: "🇭🇰 香港-01"
    type: ss
"#;
        let rule = FilterRule {
            rename_rules: vec![RenameRule {
                pattern: Regex::new(r"🇭🇰 香港-(\d+)").unwrap(),
                replacement: "HK-$1".to_string(),
            }],
            exclude_keywords: vec![Regex::new("剩余流量").unwrap()],
            exclude_types: vec!["trojan".to_string()],
            deduplication: DeduplicationStrategy::AppendIndex,
            ..Default::default()
        };
        let pipeline = SubscriptionFilterPipeline::new(rule);
        let (out, rep) = pipeline.apply_to_yaml(yaml).unwrap();

        let out_doc: Value = serde_yaml_ng::from_str(&out).unwrap();
        let proxies = out_doc.get("proxies").unwrap().as_sequence().unwrap();
        assert_eq!(proxies.len(), 2);
        assert_eq!(rep.passed, 2);
        assert_eq!(rep.excluded_by_blacklist, 1);
        assert_eq!(rep.excluded_by_type, 1);
        assert_eq!(rep.renamed, 2);
        assert_eq!(rep.deduplicated, 1);

        let n1 = proxies[0].get("name").unwrap().as_str().unwrap();
        let n2 = proxies[1].get("name").unwrap().as_str().unwrap();
        assert_eq!(n1, "HK-01");
        assert_eq!(n2, "HK-01 (1)");
    }
}
