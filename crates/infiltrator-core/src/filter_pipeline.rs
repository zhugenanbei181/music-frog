//! Node-level [`FilterPipeline`] construction and evaluation stages.

use crate::profile_converter::ProxyNodeItem;
use anyhow::Result;
use regex::Regex;
use std::collections::{HashMap, HashSet};

use super::{
    ContentDedupStrategy, DeduplicationStrategy, FilterPipeline, FilterStage, NodeMutatorConfig,
    NodeSortOrder, PipelineStats, PortFilterConfig, ServerFilterConfig, compute_node_fingerprint,
    extract_country_code, extract_multiplier, is_private_ip, normalize_country_code, strip_emojis,
};

impl FilterStage {
    pub fn regex_rename(pattern: &str, replacement: impl Into<String>) -> Result<Self> {
        Ok(Self::RegexRename {
            pattern: Regex::new(pattern)?,
            replacement: replacement.into(),
        })
    }

    pub fn multiplier_override(pattern: &str, multiplier: f64) -> Result<Self> {
        Ok(Self::MultiplierOverride {
            pattern: Regex::new(pattern)?,
            multiplier,
        })
    }

    pub fn protocol_filter<I, S>(allowed_types: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self::ProtocolFilter {
            allowed_types: allowed_types.into_iter().map(Into::into).collect(),
        }
    }

    pub fn country_code_normalizer() -> Self {
        Self::CountryCodeNormalizer
    }

    pub fn remove_emojis() -> Self {
        Self::RemoveEmojis
    }

    pub fn keyword_blacklist<I, S>(keywords: I) -> Result<Self>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let patterns = keywords
            .into_iter()
            .map(|kw| Regex::new(kw.as_ref()))
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(Self::KeywordBlacklist { patterns })
    }

    pub fn keyword_whitelist<I, S>(keywords: I) -> Result<Self>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let patterns = keywords
            .into_iter()
            .map(|kw| Regex::new(kw.as_ref()))
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(Self::KeywordWhitelist { patterns })
    }

    pub fn port_filter(allowed: Option<HashSet<u16>>, blocked: Option<HashSet<u16>>) -> Self {
        Self::PortFilter {
            config: PortFilterConfig {
                allowed_ports: allowed,
                blocked_ports: blocked,
            },
        }
    }

    pub fn server_filter(drop_private_ip: bool) -> Self {
        Self::ServerFilter {
            config: ServerFilterConfig {
                allowed_patterns: Vec::new(),
                blocked_patterns: Vec::new(),
                drop_private_ip,
            },
        }
    }

    pub fn node_mutator(config: NodeMutatorConfig) -> Self {
        Self::NodeMutator { config }
    }

    pub fn sort_nodes(order: NodeSortOrder) -> Self {
        Self::SortNodes { order }
    }

    pub fn duplicate_deduplicator(strategy: DeduplicationStrategy) -> Self {
        Self::DuplicateDeduplicator { strategy }
    }

    pub fn content_deduplicator(strategy: ContentDedupStrategy) -> Self {
        Self::ContentDeduplicator { strategy }
    }
}

impl FilterPipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new() }
    }

    pub fn with_stages(stages: Vec<FilterStage>) -> Self {
        Self { stages }
    }

    pub fn add_stage(&mut self, stage: FilterStage) -> &mut Self {
        self.stages.push(stage);
        self
    }

    pub fn stages(&self) -> &[FilterStage] {
        &self.stages
    }

    pub fn is_empty(&self) -> bool {
        self.stages.is_empty()
    }

    pub fn len(&self) -> usize {
        self.stages.len()
    }

    /// Applies the pipeline sequentially to the provided list of [`ProxyNodeItem`].
    pub fn apply_pipeline(&self, nodes: &mut Vec<ProxyNodeItem>) -> PipelineStats {
        let nodes_in = nodes.len();
        let mut renamed_count = 0usize;
        let mut mutated_count = 0usize;
        let mut deduplicated_count = 0usize;
        let mult_re =
            Regex::new(r"([\[（【])?(?:\d+(?:\.\d+)?[xX]|[xX]\d+(?:\.\d+)?)([\]）】])?").unwrap();

        for stage in &self.stages {
            match stage {
                FilterStage::RegexRename {
                    pattern,
                    replacement,
                } => {
                    for node in nodes.iter_mut() {
                        let new_name = pattern
                            .replace_all(&node.name, replacement.as_str())
                            .to_string();
                        if new_name != node.name {
                            node.name = new_name;
                            renamed_count += 1;
                        }
                    }
                }
                FilterStage::MultiplierOverride {
                    pattern,
                    multiplier,
                } => {
                    let mult_str = if (multiplier.fract()).abs() < 1e-6 {
                        format!("[{:.0}x]", multiplier)
                    } else {
                        format!("[{:.1}x]", multiplier)
                    };
                    for node in nodes.iter_mut() {
                        if pattern.is_match(&node.name) {
                            let new_name = if mult_re.is_match(&node.name) {
                                mult_re.replace(&node.name, mult_str.as_str()).to_string()
                            } else {
                                format!("{} {}", node.name.trim_end(), mult_str)
                            };
                            if new_name != node.name {
                                node.name = new_name;
                                renamed_count += 1;
                            }
                        }
                    }
                }
                FilterStage::ProtocolFilter { allowed_types } => {
                    nodes.retain(|node| {
                        allowed_types
                            .iter()
                            .any(|allowed| allowed.eq_ignore_ascii_case(&node.node_type))
                    });
                }
                FilterStage::CountryCodeNormalizer => {
                    for node in nodes.iter_mut() {
                        let normalized = normalize_country_code(&node.name);
                        if normalized != node.name {
                            node.name = normalized;
                            renamed_count += 1;
                        }
                    }
                }
                FilterStage::RemoveEmojis => {
                    for node in nodes.iter_mut() {
                        let stripped = strip_emojis(&node.name);
                        if stripped != node.name && !stripped.is_empty() {
                            node.name = stripped;
                            renamed_count += 1;
                        }
                    }
                }
                FilterStage::KeywordBlacklist { patterns } => {
                    nodes.retain(|node| !patterns.iter().any(|re| re.is_match(&node.name)));
                }
                FilterStage::KeywordWhitelist { patterns } => {
                    if !patterns.is_empty() {
                        nodes.retain(|node| patterns.iter().any(|re| re.is_match(&node.name)));
                    }
                }
                FilterStage::PortFilter { config } => {
                    nodes.retain(|node| {
                        if let Some(ref allowed) = config.allowed_ports
                            && !allowed.contains(&node.port) {
                                return false;
                            }
                        if let Some(ref blocked) = config.blocked_ports
                            && blocked.contains(&node.port) {
                                return false;
                            }
                        true
                    });
                }
                FilterStage::ServerFilter { config } => {
                    nodes.retain(|node| {
                        if config.drop_private_ip && is_private_ip(&node.server) {
                            return false;
                        }
                        if !config.allowed_patterns.is_empty()
                            && !config
                                .allowed_patterns
                                .iter()
                                .any(|re| re.is_match(&node.server))
                        {
                            return false;
                        }
                        if config
                            .blocked_patterns
                            .iter()
                            .any(|re| re.is_match(&node.server))
                        {
                            return false;
                        }
                        true
                    });
                }
                FilterStage::NodeMutator { config } => {
                    for node in nodes.iter_mut() {
                        let mut did_mutate = false;
                        if let Some(tls) = config.force_tls {
                            node.tls = tls;
                            did_mutate = true;
                        }
                        if let Some(udp) = config.force_udp {
                            node.udp = Some(udp);
                            did_mutate = true;
                        }
                        if let Some(skip) = config.skip_cert_verify {
                            node.skip_cert_verify = Some(skip);
                            did_mutate = true;
                        }
                        if let Some(ref fp) = config.client_fingerprint {
                            node.client_fingerprint = Some(fp.clone());
                            did_mutate = true;
                        }
                        if let Some(ref alpn) = config.alpn {
                            node.alpn = Some(alpn.clone());
                            did_mutate = true;
                        }
                        if let Some(tfo) = config.tfo {
                            node.tfo = Some(tfo);
                            did_mutate = true;
                        }
                        if let Some(mptcp) = config.mptcp {
                            node.mptcp = Some(mptcp);
                            did_mutate = true;
                        }
                        if let Some(smux) = config.smux {
                            if smux {
                                node.smux = Some(serde_json::json!({ "enabled": true }));
                            } else {
                                node.smux = None;
                            }
                            did_mutate = true;
                        }
                        if did_mutate {
                            mutated_count += 1;
                        }
                    }
                }
                FilterStage::SortNodes { order } => match order {
                    NodeSortOrder::Preserve => {}
                    NodeSortOrder::NameAsc => nodes.sort_by(|a, b| a.name.cmp(&b.name)),
                    NodeSortOrder::NameDesc => nodes.sort_by(|a, b| b.name.cmp(&a.name)),
                    NodeSortOrder::CountryCode => {
                        nodes.sort_by(|a, b| {
                            let ca = extract_country_code(&a.name).unwrap_or("ZZ");
                            let cb = extract_country_code(&b.name).unwrap_or("ZZ");
                            ca.cmp(cb).then_with(|| a.name.cmp(&b.name))
                        });
                    }
                    NodeSortOrder::Protocol => nodes.sort_by(|a, b| {
                        a.node_type
                            .cmp(&b.node_type)
                            .then_with(|| a.name.cmp(&b.name))
                    }),
                    NodeSortOrder::MultiplierAsc => {
                        nodes.sort_by(|a, b| {
                            let ma = extract_multiplier(&a.name).unwrap_or(1.0);
                            let mb = extract_multiplier(&b.name).unwrap_or(1.0);
                            ma.partial_cmp(&mb).unwrap_or(std::cmp::Ordering::Equal)
                        });
                    }
                    NodeSortOrder::MultiplierDesc => {
                        nodes.sort_by(|a, b| {
                            let ma = extract_multiplier(&a.name).unwrap_or(1.0);
                            let mb = extract_multiplier(&b.name).unwrap_or(1.0);
                            mb.partial_cmp(&ma).unwrap_or(std::cmp::Ordering::Equal)
                        });
                    }
                },
                FilterStage::ContentDeduplicator { strategy } => match strategy {
                    ContentDedupStrategy::Disabled => {}
                    ContentDedupStrategy::KeepFirst => {
                        let mut seen = HashSet::new();
                        nodes.retain(|node| {
                            let fp = compute_node_fingerprint(node);
                            let keep = seen.insert(fp);
                            if !keep {
                                deduplicated_count += 1;
                            }
                            keep
                        });
                    }
                    ContentDedupStrategy::KeepLast => {
                        let mut last_seen = HashMap::new();
                        for (idx, node) in nodes.iter().enumerate() {
                            let fp = compute_node_fingerprint(node);
                            last_seen.insert(fp, idx);
                        }
                        let mut idx = 0;
                        nodes.retain(|node| {
                            let fp = compute_node_fingerprint(node);
                            let keep = last_seen.get(&fp) == Some(&idx);
                            idx += 1;
                            if !keep {
                                deduplicated_count += 1;
                            }
                            keep
                        });
                    }
                    ContentDedupStrategy::KeepLowerMultiplier => {
                        let mut fp_map: HashMap<String, (usize, f64)> = HashMap::new();
                        for (idx, node) in nodes.iter().enumerate() {
                            let fp = compute_node_fingerprint(node);
                            let mult = extract_multiplier(&node.name).unwrap_or(1.0);
                            if let Some((_prev_idx, prev_mult)) = fp_map.get_mut(&fp) {
                                deduplicated_count += 1;
                                if mult < *prev_mult {
                                    *prev_mult = mult;
                                    *_prev_idx = idx;
                                }
                            } else {
                                fp_map.insert(fp, (idx, mult));
                            }
                        }
                        let keep_indices: HashSet<usize> =
                            fp_map.values().map(|(idx, _)| *idx).collect();
                        let mut idx = 0;
                        nodes.retain(|_| {
                            let keep = keep_indices.contains(&idx);
                            idx += 1;
                            keep
                        });
                    }
                },
                FilterStage::DuplicateDeduplicator { strategy } => match strategy {
                    DeduplicationStrategy::Disabled => {}
                    DeduplicationStrategy::KeepFirst => {
                        let mut seen = HashSet::new();
                        nodes.retain(|node| {
                            let keep = seen.insert(node.name.clone());
                            if !keep {
                                deduplicated_count += 1;
                            }
                            keep
                        });
                    }
                    DeduplicationStrategy::KeepLast => {
                        let mut last_seen_index = HashMap::new();
                        for (idx, node) in nodes.iter().enumerate() {
                            last_seen_index.insert(node.name.clone(), idx);
                        }
                        let mut idx = 0;
                        nodes.retain(|node| {
                            let keep = last_seen_index.get(&node.name) == Some(&idx);
                            idx += 1;
                            if !keep {
                                deduplicated_count += 1;
                            }
                            keep
                        });
                    }
                    DeduplicationStrategy::AppendIndex => {
                        let mut seen_count: HashMap<String, usize> = HashMap::new();
                        let mut seen_names: HashSet<String> = HashSet::new();
                        for node in nodes.iter_mut() {
                            let base_name = node.name.clone();
                            let entry = seen_count.entry(base_name.clone()).or_insert(0);
                            *entry += 1;
                            if *entry > 1 || seen_names.contains(&node.name) {
                                let mut count = *entry - 1;
                                let mut candidate = format!("{base_name} ({count})");
                                while seen_names.contains(&candidate) {
                                    count += 1;
                                    candidate = format!("{base_name} ({count})");
                                }
                                node.name = candidate.clone();
                                seen_names.insert(candidate);
                                renamed_count += 1;
                                deduplicated_count += 1;
                            } else {
                                seen_names.insert(node.name.clone());
                            }
                        }
                    }
                },
            }
        }

        let nodes_out = nodes.len();
        let dropped_count = nodes_in.saturating_sub(nodes_out);
        PipelineStats {
            nodes_in,
            nodes_out,
            renamed_count,
            dropped_count,
            mutated_count,
            deduplicated_count,
        }
    }
}
