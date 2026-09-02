//! YAML-level subscription filtering and proxy-name evaluation.

use anyhow::{Context, Result};
use regex::Regex;
use serde_yaml_ng::Value;
use std::collections::{HashMap, HashSet};

use super::{
    ContentDedupStrategy, DeduplicationStrategy, FilterReport, FilterRule, NodeSortOrder,
    SubscriptionFilterPipeline, extract_country_code, extract_multiplier, is_private_ip,
    normalize_country_code, strip_emojis,
};

impl SubscriptionFilterPipeline {
    pub fn new(rule: FilterRule) -> Self {
        Self { rule }
    }

    pub fn apply_to_yaml(&self, yaml_str: &str) -> Result<(String, FilterReport)> {
        let mut doc: Value = serde_yaml_ng::from_str(yaml_str).context("Failed to parse YAML")?;
        let mut report = FilterReport::default();

        if let Some(proxies_seq) = doc.get_mut("proxies").and_then(Value::as_sequence_mut) {
            report.total_input = proxies_seq.len();
            let mut new_proxies = Vec::new();
            let mut seen_names = HashSet::new();
            let mut last_seen_index = HashMap::new();
            let mut seen_fingerprints = HashMap::new();
            let mult_re =
                Regex::new(r"([\[（【])?(?:\d+(?:\.\d+)?[xX]|[xX]\d+(?:\.\d+)?)([\]）】])?")
                    .unwrap();

            for proxy in proxies_seq.iter() {
                let Some(map) = proxy.as_mapping() else {
                    continue;
                };
                let name = map
                    .get(Value::String("name".into()))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let ptype = map
                    .get(Value::String("type".into()))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                let server = map
                    .get(Value::String("server".into()))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                let port = map
                    .get(Value::String("port".into()))
                    .and_then(Value::as_u64)
                    .unwrap_or(0) as u16;

                if self
                    .rule
                    .exclude_types
                    .iter()
                    .any(|t| t.eq_ignore_ascii_case(ptype))
                {
                    report.excluded_by_type += 1;
                    continue;
                }
                if let Some(ref allowed) = self.rule.allowed_ports {
                    if !allowed.contains(&port) {
                        report.excluded_by_port += 1;
                        continue;
                    }
                }
                if let Some(ref blocked) = self.rule.blocked_ports {
                    if blocked.contains(&port) {
                        report.excluded_by_port += 1;
                        continue;
                    }
                }
                if self.rule.drop_private_ip && is_private_ip(server) {
                    report.excluded_by_server += 1;
                    continue;
                }
                if !self.rule.include_keywords.is_empty()
                    && !self.rule.include_keywords.iter().any(|r| r.is_match(&name))
                {
                    report.excluded_by_whitelist += 1;
                    continue;
                }
                if self.rule.exclude_keywords.iter().any(|r| r.is_match(&name)) {
                    report.excluded_by_blacklist += 1;
                    continue;
                }

                let mut cur_name = name;
                let mut was_renamed = false;

                // 1. Rename rules
                for rule in &self.rule.rename_rules {
                    let next = rule
                        .pattern
                        .replace_all(&cur_name, &rule.replacement)
                        .to_string();
                    if next != cur_name {
                        cur_name = next;
                        was_renamed = true;
                    }
                }

                // 2. Remove emojis if enabled
                if self.rule.remove_emojis {
                    let stripped = strip_emojis(&cur_name);
                    if stripped != cur_name && !stripped.is_empty() {
                        cur_name = stripped;
                        was_renamed = true;
                    }
                }

                // 3. Country code normalizer if enabled
                if self.rule.normalize_country_code {
                    let normalized = normalize_country_code(&cur_name);
                    if normalized != cur_name {
                        cur_name = normalized;
                        was_renamed = true;
                    }
                }

                // 4. Multiplier overrides
                for m_rule in &self.rule.multiplier_rules {
                    if m_rule.pattern.is_match(&cur_name) {
                        let mult_str = if (m_rule.multiplier.fract()).abs() < 1e-6 {
                            format!("[{:.0}x]", m_rule.multiplier)
                        } else {
                            format!("[{:.1}x]", m_rule.multiplier)
                        };
                        let next = if mult_re.is_match(&cur_name) {
                            mult_re.replace(&cur_name, mult_str.as_str()).to_string()
                        } else {
                            format!("{} {}", cur_name.trim_end(), mult_str)
                        };
                        if next != cur_name {
                            cur_name = next;
                            was_renamed = true;
                        }
                    }
                }

                if was_renamed {
                    report.renamed += 1;
                }

                // Content fingerprint deduplication check
                let mut fp = format!("{ptype}:{server}:{port}");
                if let Some(pw) = map
                    .get(Value::String("password".into()))
                    .and_then(Value::as_str)
                {
                    fp.push(':');
                    fp.push_str(pw);
                }
                if let Some(uuid) = map
                    .get(Value::String("uuid".into()))
                    .and_then(Value::as_str)
                {
                    fp.push(':');
                    fp.push_str(uuid);
                }

                match self.rule.content_dedup {
                    ContentDedupStrategy::Disabled => {}
                    ContentDedupStrategy::KeepFirst => {
                        if seen_fingerprints.contains_key(&fp) {
                            report.deduplicated += 1;
                            continue;
                        }
                        seen_fingerprints.insert(fp.clone(), new_proxies.len());
                    }
                    ContentDedupStrategy::KeepLast => {
                        if let Some(&idx) = seen_fingerprints.get(&fp) {
                            report.deduplicated += 1;
                            new_proxies[idx] = Value::Null;
                        }
                        seen_fingerprints.insert(fp.clone(), new_proxies.len());
                    }
                    ContentDedupStrategy::KeepLowerMultiplier => {
                        let cur_mult = extract_multiplier(&cur_name).unwrap_or(1.0);
                        if let Some(&idx) = seen_fingerprints.get(&fp) {
                            report.deduplicated += 1;
                            let prev_name = new_proxies[idx]
                                .as_mapping()
                                .and_then(|m| m.get(Value::String("name".into())))
                                .and_then(Value::as_str)
                                .unwrap_or("");
                            let prev_mult = extract_multiplier(prev_name).unwrap_or(1.0);
                            if cur_mult < prev_mult {
                                new_proxies[idx] = Value::Null;
                                seen_fingerprints.insert(fp.clone(), new_proxies.len());
                            } else {
                                continue;
                            }
                        } else {
                            seen_fingerprints.insert(fp.clone(), new_proxies.len());
                        }
                    }
                }

                // Name deduplication check
                match self.rule.deduplication {
                    DeduplicationStrategy::Disabled => {}
                    DeduplicationStrategy::KeepFirst => {
                        if !seen_names.insert(cur_name.clone()) {
                            report.deduplicated += 1;
                            continue;
                        }
                    }
                    DeduplicationStrategy::KeepLast => {
                        if let Some(idx) = last_seen_index.get(&cur_name) {
                            report.deduplicated += 1;
                            new_proxies[*idx] = Value::Null;
                        }
                        last_seen_index.insert(cur_name.clone(), new_proxies.len());
                    }
                    DeduplicationStrategy::AppendIndex => {
                        let mut count = 1;
                        let base = cur_name.clone();
                        while seen_names.contains(&cur_name) {
                            cur_name = format!("{base} ({count})");
                            count += 1;
                        }
                        if count > 1 {
                            report.deduplicated += 1;
                        }
                        seen_names.insert(cur_name.clone());
                    }
                }

                let mut p = proxy.clone();
                if let Some(m) = p.as_mapping_mut() {
                    m.insert(Value::String("name".into()), Value::String(cur_name));

                    // Node mutation
                    if let Some(ref mutator) = self.rule.node_mutator {
                        let mut did_mutate = false;
                        if let Some(tls) = mutator.force_tls {
                            m.insert(Value::String("tls".into()), Value::Bool(tls));
                            did_mutate = true;
                        }
                        if let Some(udp) = mutator.force_udp {
                            m.insert(Value::String("udp".into()), Value::Bool(udp));
                            did_mutate = true;
                        }
                        if let Some(skip) = mutator.skip_cert_verify {
                            m.insert(Value::String("skip-cert-verify".into()), Value::Bool(skip));
                            did_mutate = true;
                        }
                        if let Some(ref fp) = mutator.client_fingerprint {
                            m.insert(
                                Value::String("client-fingerprint".into()),
                                Value::String(fp.clone()),
                            );
                            did_mutate = true;
                        }
                        if let Some(tfo) = mutator.tfo {
                            m.insert(Value::String("tfo".into()), Value::Bool(tfo));
                            did_mutate = true;
                        }
                        if let Some(mptcp) = mutator.mptcp {
                            m.insert(Value::String("mptcp".into()), Value::Bool(mptcp));
                            did_mutate = true;
                        }
                        if did_mutate {
                            report.mutated += 1;
                        }
                    }
                }
                new_proxies.push(p);
            }

            if self.rule.deduplication == DeduplicationStrategy::KeepLast
                || self.rule.content_dedup == ContentDedupStrategy::KeepLast
                || self.rule.content_dedup == ContentDedupStrategy::KeepLowerMultiplier
            {
                new_proxies.retain(|p| !p.is_null());
            }

            // Sorting if requested
            match self.rule.sort_order {
                NodeSortOrder::Preserve => {}
                NodeSortOrder::NameAsc => {
                    new_proxies.sort_by(|a, b| {
                        let na = a
                            .as_mapping()
                            .and_then(|m| m.get(Value::String("name".into())))
                            .and_then(Value::as_str)
                            .unwrap_or("");
                        let nb = b
                            .as_mapping()
                            .and_then(|m| m.get(Value::String("name".into())))
                            .and_then(Value::as_str)
                            .unwrap_or("");
                        na.cmp(nb)
                    });
                }
                NodeSortOrder::NameDesc => {
                    new_proxies.sort_by(|a, b| {
                        let na = a
                            .as_mapping()
                            .and_then(|m| m.get(Value::String("name".into())))
                            .and_then(Value::as_str)
                            .unwrap_or("");
                        let nb = b
                            .as_mapping()
                            .and_then(|m| m.get(Value::String("name".into())))
                            .and_then(Value::as_str)
                            .unwrap_or("");
                        nb.cmp(na)
                    });
                }
                NodeSortOrder::CountryCode => {
                    new_proxies.sort_by(|a, b| {
                        let na = a
                            .as_mapping()
                            .and_then(|m| m.get(Value::String("name".into())))
                            .and_then(Value::as_str)
                            .unwrap_or("");
                        let nb = b
                            .as_mapping()
                            .and_then(|m| m.get(Value::String("name".into())))
                            .and_then(Value::as_str)
                            .unwrap_or("");
                        let ca = extract_country_code(na).unwrap_or("ZZ");
                        let cb = extract_country_code(nb).unwrap_or("ZZ");
                        ca.cmp(cb).then_with(|| na.cmp(nb))
                    });
                }
                NodeSortOrder::Protocol => {
                    new_proxies.sort_by(|a, b| {
                        let ta = a
                            .as_mapping()
                            .and_then(|m| m.get(Value::String("type".into())))
                            .and_then(Value::as_str)
                            .unwrap_or("");
                        let tb = b
                            .as_mapping()
                            .and_then(|m| m.get(Value::String("type".into())))
                            .and_then(Value::as_str)
                            .unwrap_or("");
                        ta.cmp(tb)
                    });
                }
                NodeSortOrder::MultiplierAsc => {
                    new_proxies.sort_by(|a, b| {
                        let na = a
                            .as_mapping()
                            .and_then(|m| m.get(Value::String("name".into())))
                            .and_then(Value::as_str)
                            .unwrap_or("");
                        let nb = b
                            .as_mapping()
                            .and_then(|m| m.get(Value::String("name".into())))
                            .and_then(Value::as_str)
                            .unwrap_or("");
                        let ma = extract_multiplier(na).unwrap_or(1.0);
                        let mb = extract_multiplier(nb).unwrap_or(1.0);
                        ma.partial_cmp(&mb).unwrap_or(std::cmp::Ordering::Equal)
                    });
                }
                NodeSortOrder::MultiplierDesc => {
                    new_proxies.sort_by(|a, b| {
                        let na = a
                            .as_mapping()
                            .and_then(|m| m.get(Value::String("name".into())))
                            .and_then(Value::as_str)
                            .unwrap_or("");
                        let nb = b
                            .as_mapping()
                            .and_then(|m| m.get(Value::String("name".into())))
                            .and_then(Value::as_str)
                            .unwrap_or("");
                        let ma = extract_multiplier(na).unwrap_or(1.0);
                        let mb = extract_multiplier(nb).unwrap_or(1.0);
                        mb.partial_cmp(&ma).unwrap_or(std::cmp::Ordering::Equal)
                    });
                }
            }

            report.passed = new_proxies.len();
            *proxies_seq = new_proxies;
        }

        Ok((serde_yaml_ng::to_string(&doc)?, report))
    }

    pub fn filter_proxy_names(&self, names: &[String]) -> (Vec<String>, FilterReport) {
        let mut report = FilterReport {
            total_input: names.len(),
            ..Default::default()
        };
        let mut out_names = Vec::new();
        let mut seen_names = HashSet::new();
        let mut last_seen_index = HashMap::new();

        for name in names {
            if !self.rule.include_keywords.is_empty()
                && !self.rule.include_keywords.iter().any(|r| r.is_match(name))
            {
                report.excluded_by_whitelist += 1;
                continue;
            }
            if self.rule.exclude_keywords.iter().any(|r| r.is_match(name)) {
                report.excluded_by_blacklist += 1;
                continue;
            }

            let mut cur_name = name.clone();
            let mut was_renamed = false;
            for rule in &self.rule.rename_rules {
                let next = rule
                    .pattern
                    .replace_all(&cur_name, &rule.replacement)
                    .to_string();
                if next != cur_name {
                    cur_name = next;
                    was_renamed = true;
                }
            }
            if self.rule.remove_emojis {
                let stripped = strip_emojis(&cur_name);
                if stripped != cur_name && !stripped.is_empty() {
                    cur_name = stripped;
                    was_renamed = true;
                }
            }
            if self.rule.normalize_country_code {
                let normalized = normalize_country_code(&cur_name);
                if normalized != cur_name {
                    cur_name = normalized;
                    was_renamed = true;
                }
            }
            if was_renamed {
                report.renamed += 1;
            }

            match self.rule.deduplication {
                DeduplicationStrategy::Disabled => {}
                DeduplicationStrategy::KeepFirst => {
                    if !seen_names.insert(cur_name.clone()) {
                        report.deduplicated += 1;
                        continue;
                    }
                }
                DeduplicationStrategy::KeepLast => {
                    if let Some(idx) = last_seen_index.get(&cur_name) {
                        report.deduplicated += 1;
                        out_names[*idx] = String::new();
                    }
                    last_seen_index.insert(cur_name.clone(), out_names.len());
                }
                DeduplicationStrategy::AppendIndex => {
                    let mut count = 1;
                    let base = cur_name.clone();
                    while seen_names.contains(&cur_name) {
                        cur_name = format!("{base} ({count})");
                        count += 1;
                    }
                    if count > 1 {
                        report.deduplicated += 1;
                    }
                    seen_names.insert(cur_name.clone());
                }
            }
            out_names.push(cur_name);
        }

        if self.rule.deduplication == DeduplicationStrategy::KeepLast {
            out_names.retain(|n| !n.is_empty());
        }
        report.passed = out_names.len();
        (out_names, report)
    }
}
