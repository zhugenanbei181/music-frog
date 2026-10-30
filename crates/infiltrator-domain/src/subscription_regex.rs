//! Regex-based node name normalization and filtering pipeline (FLPARITY-033).
//!
//! Provides compiled regex transformation pipelines for renaming, stripping prefixes/multipliers,
//! and filtering out announcement or non-proxy nodes before committing them to subscription proxies.

use infiltrator_contract::subscription_diagnostic::{NodeRegexRule, NodeRegexTransformPipeline};
use regex::Regex;

/// A single compiled regex rule for node transformation.
#[derive(Debug, Clone)]
pub struct CompiledNodeRegexRule {
    pub pattern: String,
    pub compiled: Result<Regex, String>,
    pub replacement: String,
    pub is_exclude: bool,
}

impl CompiledNodeRegexRule {
    pub fn from_rule(rule: &NodeRegexRule) -> Self {
        let compiled = Regex::new(&rule.pattern).map_err(|e| e.to_string());
        Self {
            pattern: rule.pattern.clone(),
            compiled,
            replacement: rule.replacement.clone(),
            is_exclude: rule.is_exclude,
        }
    }

    /// Check if pattern compiled cleanly.
    pub fn is_valid(&self) -> bool {
        self.compiled.is_ok()
    }
}

/// Preview inspection result for a single node under the regex pipeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeRegexPreviewItem {
    pub original: String,
    pub result: Option<String>,
    pub is_excluded: bool,
    pub is_modified: bool,
    pub matched_rule_indices: Vec<usize>,
}

/// Fully compiled multi-stage regex transform pipeline.
#[derive(Debug, Clone)]
pub struct CompiledNodeRegexPipeline {
    pub rules: Vec<CompiledNodeRegexRule>,
}

impl CompiledNodeRegexPipeline {
    /// Compile a declarative `NodeRegexTransformPipeline`.
    pub fn compile(pipeline: &NodeRegexTransformPipeline) -> Self {
        let rules = pipeline
            .rules
            .iter()
            .map(CompiledNodeRegexRule::from_rule)
            .collect();
        Self { rules }
    }

    /// Are all rules in the pipeline syntactically valid?
    pub fn is_all_valid(&self) -> bool {
        self.rules.iter().all(|r| r.is_valid())
    }

    /// Apply transformation pipeline to a single node name.
    /// Returns `None` if the node is excluded/dropped by an exclusion rule.
    pub fn apply_to_name(&self, name: &str) -> Option<String> {
        let mut current = name.to_owned();
        for rule in &self.rules {
            if let Ok(re) = &rule.compiled {
                if rule.is_exclude {
                    if re.is_match(&current) {
                        return None;
                    }
                } else if re.is_match(&current) {
                    current = re.replace_all(&current, &rule.replacement).into_owned();
                }
            }
        }
        Some(current)
    }

    /// Apply pipeline to a batch of node names, dropping excluded nodes.
    pub fn apply_batch(&self, names: &[impl AsRef<str>]) -> Vec<String> {
        names
            .iter()
            .filter_map(|n| self.apply_to_name(n.as_ref()))
            .collect()
    }

    /// Produce a detailed preview of transformations on sample names.
    pub fn preview(&self, samples: &[&str]) -> Vec<NodeRegexPreviewItem> {
        samples
            .iter()
            .map(|&orig| {
                let mut current = orig.to_owned();
                let mut matched_indices = Vec::new();
                let mut excluded = false;

                for (idx, rule) in self.rules.iter().enumerate() {
                    if let Ok(re) = &rule.compiled
                        && re.is_match(&current)
                    {
                        matched_indices.push(idx);
                        if rule.is_exclude {
                            excluded = true;
                            break;
                        }
                        current = re.replace_all(&current, &rule.replacement).into_owned();
                    }
                }

                NodeRegexPreviewItem {
                    original: orig.to_owned(),
                    result: if excluded {
                        None
                    } else {
                        Some(current.clone())
                    },
                    is_excluded: excluded,
                    is_modified: !excluded && current != orig,
                    matched_rule_indices: matched_indices,
                }
            })
            .collect()
    }

    /// Preset: remove traffic multipliers like `[1.5x]` or `(2.0倍)`.
    pub fn preset_strip_multiplier() -> NodeRegexRule {
        NodeRegexRule::rename(
            r"\[\s*\d+(\.\d+)?\s*[xX倍]\s*\]|\(\s*\d+(\.\d+)?\s*[xX倍]\s*\)",
            "",
        )
    }

    /// Preset: drop non-node announcements (e.g. 官网, 发布页, 到期, 重置).
    pub fn preset_exclude_announcements() -> NodeRegexRule {
        NodeRegexRule::exclude(r"(?i)官网|发布页|重置|剩余|到期|通知|群|频道|客服|套餐|维护")
    }

    /// Preset: strip leading provider tags like `[机场名]` or `【专线】`.
    pub fn preset_strip_provider_tags() -> NodeRegexRule {
        NodeRegexRule::rename(r"^(\s*\[[^\]]+\]|\s*\([^\)]+\)|\s*【[^】]+】)\s*", "")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipeline_renames_and_excludes_nodes() {
        let rules = vec![
            CompiledNodeRegexPipeline::preset_strip_provider_tags(),
            CompiledNodeRegexPipeline::preset_strip_multiplier(),
            CompiledNodeRegexPipeline::preset_exclude_announcements(),
            NodeRegexRule::rename(r"\s+", " "),
        ];

        let pipeline = NodeRegexTransformPipeline::new(rules);
        let compiled = CompiledNodeRegexPipeline::compile(&pipeline);
        assert!(compiled.is_all_valid());

        let samples = [
            "[MyAirport] 🇭🇰 香港 01 [1.5x]",
            "【VIP】 🇯🇵 日本 02 (2.0倍)",
            "📣 官网地址: https://myairport.com",
            "🇺🇸 美国 03 [0.5x]   高速",
        ];

        let preview = compiled.preview(&samples);
        assert_eq!(preview.len(), 4);

        // 1. Tag and multiplier stripped
        assert_eq!(preview[0].result.as_deref(), Some("🇭🇰 香港 01 "));
        assert!(!preview[0].is_excluded);
        assert!(preview[0].is_modified);

        // 2. Tag and multiplier stripped
        assert_eq!(preview[1].result.as_deref(), Some("🇯🇵 日本 02 "));
        assert!(!preview[1].is_excluded);

        // 3. Announcement node dropped
        assert!(preview[2].is_excluded);
        assert_eq!(preview[2].result, None);

        // Batch test drops announcement
        let batch = compiled.apply_batch(&samples);
        assert_eq!(batch.len(), 3);
        assert!(!batch.iter().any(|n| n.contains("官网")));
    }
}
