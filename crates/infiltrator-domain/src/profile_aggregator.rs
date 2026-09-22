//! DUAL-08 multi-subscription aggregation: structured dedup, geo clustering
//! and proxy-group cascade synthesis.
//!
//! [`ProfileAggregator::plan`] is the single source of truth behind
//! [`crate::profile_converter::MultiSubscriptionAggregator::aggregate`]. It
//! merges the selected source documents, optionally drops the nodes failing
//! the required-field precheck, runs the user's rename rules first and then
//! the shared [`FilterPipeline`] (content-fingerprint dedup, name dedup,
//! country-code normalisation, emoji removal, sorting), clusters the
//! surviving nodes by ISO region, synthesises the proxy-group cascade (region
//! groups plus the user-authored groups), and renders the mihomo YAML
//! document.
//!
//! Unlike the legacy string-only aggregate, the plan surfaces the per-stage
//! counters and the derived topology, so both surfaces can render real dedup
//! facts and real regional groups instead of fabricated placeholders.

use anyhow::{Result, anyhow, bail};
use std::collections::{BTreeMap, BTreeSet};

use crate::filter::{
    ContentDedupStrategy, DeduplicationStrategy, FilterPipeline, FilterStage, NodeSortOrder,
    extract_country_code,
};
use crate::profile_converter::{
    AggregationOptions, ProfileConverter, ProfileFormat, ProxyNodeItem, SourceSubscription,
};

#[cfg(test)]
#[path = "profile_aggregator_test.rs"]
mod profile_aggregator_test;

/// Bounded number of precheck findings surfaced in the report (DUAL-08-09).
pub const MAX_PRECHECK_SAMPLES: usize = 8;
/// Proxy-group types a user-authored custom group may use (DUAL-08-10).
pub const CUSTOM_GROUP_TYPES: [&str; 2] = ["select", "url-test"];

/// Default health-check URL used by every generated `url-test` group.
pub const AGGREGATION_HEALTH_CHECK_URL: &str = "http://www.gstatic.com/generate_204";
/// Default `url-test` probe interval in seconds.
pub const AGGREGATION_HEALTH_CHECK_INTERVAL: u64 = 300;
/// Default `url-test` tolerance in milliseconds.
pub const AGGREGATION_HEALTH_CHECK_TOLERANCE: u64 = 50;
/// Master selection group that cascades the regional groups.
pub const MASTER_SELECT_GROUP: &str = "🚀 节点选择";
/// Global automatic `url-test` group over every surviving node.
pub const AUTO_SELECT_GROUP: &str = "♻️ 自动选择";
/// Direct-routing group.
pub const DIRECT_GROUP: &str = "🎯 全球直连";
/// Ad-reject group.
pub const REJECT_GROUP: &str = "🛑 广告拦截";
/// Suffix appended to a region label to form its `url-test` group name.
pub const REGION_GROUP_SUFFIX: &str = "自动测速";

/// One ISO region cluster produced by geo clustering (DUAL-08-03).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionalCluster {
    /// ISO-3166 alpha-2 code, e.g. `HK`.
    pub iso: String,
    /// Human label for the region, e.g. `香港`.
    pub label: String,
    /// Region flag derived from the ISO code, e.g. `🇭🇰`.
    pub flag: String,
    /// Names of the nodes assigned to this cluster, in report order.
    pub node_names: Vec<String>,
}

impl RegionalCluster {
    /// DUAL-08-04: the generated `url-test` group name for this region.
    pub fn group_name(&self) -> String {
        format!("{}{REGION_GROUP_SUFFIX}", self.label)
    }
}

/// One synthesized proxy group in the cascade topology (DUAL-08-04/08-05).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedGroup {
    /// Group name as written into the profile.
    pub name: String,
    /// Mihomo group type: `select` or `url-test`.
    pub group_type: String,
    /// Whether this is the master selector that cascades the region groups.
    pub is_master: bool,
    /// DUAL-08-10: whether this group came from the user-authored topology.
    pub is_custom: bool,
    /// Member names (region-group references and/or concrete nodes).
    pub members: Vec<String>,
}

/// Structured aggregation result shared by the application and both surfaces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AggregationPlan {
    /// Number of source subscriptions that contributed to this plan.
    pub source_count: usize,
    /// Node count before any cleaning stage ran.
    pub input_nodes: usize,
    /// Node count after the cleaning pipeline.
    pub total_nodes: usize,
    /// Nodes removed by fingerprint/name deduplication.
    pub duplicates_removed: usize,
    /// Nodes renamed by country-code normalisation.
    pub renamed_nodes: usize,
    /// DUAL-08-08: nodes renamed by the user's regex rules.
    pub rule_renamed_nodes: usize,
    /// DUAL-08-09: nodes dropped by the required-field precheck.
    pub invalid_nodes_removed: usize,
    /// DUAL-08-09: bounded sample of the dropped-node findings.
    pub invalid_node_samples: Vec<String>,
    /// ISO region clusters derived from the surviving nodes.
    pub regions: Vec<RegionalCluster>,
    /// Synthesized cascade groups (empty when group generation is disabled).
    pub groups: Vec<GeneratedGroup>,
    /// Rendered mihomo YAML document.
    pub yaml: String,
}

/// Structured aggregator over multiple source subscriptions.
pub struct ProfileAggregator;

impl ProfileAggregator {
    /// Merge, clean, cluster and synthesize the plan for `sources`.
    pub fn plan(
        sources: &[SourceSubscription],
        options: &AggregationOptions,
    ) -> Result<AggregationPlan> {
        Self::validate_options(options)?;

        let mut all_nodes: Vec<ProxyNodeItem> = Vec::new();
        let mut contributing_sources = 0usize;
        let mut merged_nodes = 0usize;
        let mut invalid_nodes_removed = 0usize;
        let mut invalid_node_samples: Vec<String> = Vec::new();

        for source in sources {
            let converted = ProfileConverter::detect_and_convert(&source.content)?;
            let mut nodes = ProfileConverter::parse_nodes(&converted, ProfileFormat::ClashYaml)?;
            if nodes.is_empty() {
                continue;
            }
            contributing_sources += 1;
            if let Some(ref pfx) = source.prefix {
                let tag = format!("[{pfx}]");
                for node in &mut nodes {
                    if !node.name.starts_with(&tag) {
                        node.name = format!("{tag} {}", node.name.trim());
                    }
                }
            }
            merged_nodes += nodes.len();
            if options.availability_precheck {
                nodes = Self::precheck_nodes(
                    nodes,
                    &converted,
                    &mut invalid_nodes_removed,
                    &mut invalid_node_samples,
                );
            }
            all_nodes.extend(nodes);
        }

        if all_nodes.is_empty() && invalid_nodes_removed == 0 {
            return Err(anyhow!(
                "No valid proxy nodes found across sources to aggregate"
            ));
        }
        if all_nodes.is_empty() {
            bail!("all {invalid_nodes_removed} nodes failed the availability precheck");
        }

        let input_nodes = merged_nodes;
        let rule_renamed_nodes = Self::rename_pipeline(options)?
            .apply_pipeline(&mut all_nodes)
            .renamed_count;
        let stats = Self::pipeline(options).apply_pipeline(&mut all_nodes);
        let regions = Self::cluster_regions(&all_nodes);
        let groups = if options.generate_proxy_groups {
            Self::validate_custom_groups(&regions, options)?;
            Self::synthesize_groups(&regions, &all_nodes, &options.custom_groups)
        } else {
            Vec::new()
        };
        let yaml = Self::render_yaml(&all_nodes, &groups)?;

        Ok(AggregationPlan {
            source_count: contributing_sources,
            input_nodes,
            total_nodes: all_nodes.len(),
            duplicates_removed: stats.deduplicated_count,
            renamed_nodes: stats.renamed_count,
            rule_renamed_nodes,
            invalid_nodes_removed,
            invalid_node_samples,
            regions,
            groups,
            yaml,
        })
    }

    /// DUAL-08-09: drop the nodes that fail the required-field precheck,
    /// recording how many were removed and a bounded sample of findings.
    ///
    /// The rich typed model is paired by index when both readers walk the same
    /// `proxies:` list; a node without a rich counterpart is still checked by
    /// the flat [`crate::proxy_nodes::validate::validate_item`] rules.
    fn precheck_nodes(
        nodes: Vec<ProxyNodeItem>,
        converted: &str,
        removed: &mut usize,
        samples: &mut Vec<String>,
    ) -> Vec<ProxyNodeItem> {
        let rich = crate::proxy_nodes::profile_yaml::parse_profile_yaml(converted)
            .ok()
            .filter(|rich| rich.len() == nodes.len());
        let mut kept = Vec::with_capacity(nodes.len());
        for (index, node) in nodes.into_iter().enumerate() {
            let mut issues = crate::proxy_nodes::validate::validate_item(&node);
            if let Some(rich) = &rich {
                issues.extend(crate::proxy_nodes::validate::validate(&rich[index]));
            }
            if issues.is_empty() {
                kept.push(node);
                continue;
            }
            *removed += 1;
            if samples.len() < MAX_PRECHECK_SAMPLES {
                let name = if node.name.trim().is_empty() {
                    "(unnamed)".to_owned()
                } else {
                    node.name.clone()
                };
                samples.push(format!("{name}: {}", issues[0]));
            }
        }
        kept
    }

    /// Reject custom-group names that would collide with the synthesized
    /// topology before any group is written (DUAL-08-10).
    fn validate_custom_groups(
        regions: &[RegionalCluster],
        options: &AggregationOptions,
    ) -> Result<()> {
        let mut reserved: BTreeSet<String> = [
            MASTER_SELECT_GROUP,
            AUTO_SELECT_GROUP,
            DIRECT_GROUP,
            REJECT_GROUP,
        ]
        .into_iter()
        .map(str::to_owned)
        .collect();
        reserved.extend(regions.iter().map(RegionalCluster::group_name));
        for group in &options.custom_groups {
            if !reserved.insert(group.name.trim().to_owned()) {
                bail!(
                    "custom group `{}` collides with an existing proxy group",
                    group.name
                );
            }
        }
        Ok(())
    }

    /// DUAL-08-08/08-10: reject option values the pipeline cannot honour
    /// instead of silently ignoring them.
    fn validate_options(options: &AggregationOptions) -> Result<()> {
        for (index, group) in options.custom_groups.iter().enumerate() {
            if group.name.trim().is_empty() {
                bail!("custom group #{} needs a name", index + 1);
            }
            let group_type = group.group_type.trim();
            if !CUSTOM_GROUP_TYPES.contains(&group_type) {
                bail!(
                    "custom group `{}` has unsupported type `{}` (expected one of {:?})",
                    group.name,
                    group_type,
                    CUSTOM_GROUP_TYPES
                );
            }
        }
        Ok(())
    }

    /// DUAL-08-08: the user-authored rename stages, applied before the shared
    /// cleaning pipeline so country-code normalisation and dedup see the
    /// renamed values. Invalid patterns surface as an actionable error.
    pub fn rename_pipeline(options: &AggregationOptions) -> Result<FilterPipeline> {
        let mut pipeline = FilterPipeline::new();
        for (index, rule) in options.rename_rules.iter().enumerate() {
            pipeline.add_stage(
                FilterStage::regex_rename(&rule.pattern, rule.replacement.clone()).map_err(
                    |error| anyhow!("rename rule #{} ({:?}): {error}", index + 1, rule.pattern),
                )?,
            );
        }
        Ok(pipeline)
    }

    /// The shared cleaning pipeline: the exact stage order both surfaces and
    /// the legacy string aggregate consume.
    pub fn pipeline(options: &AggregationOptions) -> FilterPipeline {
        let mut pipeline = FilterPipeline::new();
        if options.remove_emojis {
            pipeline.add_stage(FilterStage::remove_emojis());
        }
        if options.normalize_country_code {
            pipeline.add_stage(FilterStage::country_code_normalizer());
        }
        if options.content_dedup != ContentDedupStrategy::Disabled {
            pipeline.add_stage(FilterStage::content_deduplicator(options.content_dedup));
        }
        if options.name_dedup != DeduplicationStrategy::Disabled {
            pipeline.add_stage(FilterStage::duplicate_deduplicator(options.name_dedup));
        }
        if options.sort_by != NodeSortOrder::Preserve {
            pipeline.add_stage(FilterStage::sort_nodes(options.sort_by));
        }
        pipeline
    }

    /// DUAL-08-03: cluster surviving nodes by ISO country code. Nodes whose
    /// name carries no recognized region are intentionally left out of every
    /// cluster rather than bucketed into a fabricated "other" region.
    pub fn cluster_regions(nodes: &[ProxyNodeItem]) -> Vec<RegionalCluster> {
        let mut by_iso: BTreeMap<&'static str, Vec<String>> = BTreeMap::new();
        for node in nodes {
            if let Some(iso) = extract_country_code(&node.name) {
                by_iso.entry(iso).or_default().push(node.name.clone());
            }
        }
        by_iso
            .into_iter()
            .map(|(iso, node_names)| RegionalCluster {
                iso: iso.to_owned(),
                label: region_label(iso),
                flag: region_flag(iso),
                node_names,
            })
            .collect()
    }

    /// DUAL-08-04/08-05/08-10: synthesize the master selector, the global auto
    /// `url-test`, one `url-test` per region, the user-authored groups, and the
    /// direct/reject groups.
    pub fn synthesize_groups(
        regions: &[RegionalCluster],
        nodes: &[ProxyNodeItem],
        custom_groups: &[infiltrator_contract::aggregator::AggregationCustomGroup],
    ) -> Vec<GeneratedGroup> {
        let node_names: Vec<String> = nodes.iter().map(|node| node.name.clone()).collect();

        // DUAL-08-05: the master selector cascades `auto`, `direct`, every
        // regional group, every custom group, then the concrete nodes.
        let mut master_members = vec![AUTO_SELECT_GROUP.to_owned(), DIRECT_GROUP.to_owned()];
        master_members.extend(regions.iter().map(RegionalCluster::group_name));
        master_members.extend(
            custom_groups
                .iter()
                .map(|group| group.name.trim().to_owned()),
        );
        master_members.extend(node_names.iter().cloned());

        let mut groups = vec![
            GeneratedGroup {
                name: MASTER_SELECT_GROUP.to_owned(),
                group_type: "select".to_owned(),
                is_master: true,
                is_custom: false,
                members: master_members,
            },
            GeneratedGroup {
                name: AUTO_SELECT_GROUP.to_owned(),
                group_type: "url-test".to_owned(),
                is_master: false,
                is_custom: false,
                members: node_names.clone(),
            },
        ];

        for region in regions {
            groups.push(GeneratedGroup {
                name: region.group_name(),
                group_type: "url-test".to_owned(),
                is_master: false,
                is_custom: false,
                members: region.node_names.clone(),
            });
        }

        for custom in custom_groups {
            groups.push(GeneratedGroup {
                name: custom.name.trim().to_owned(),
                group_type: custom.group_type.trim().to_owned(),
                is_master: false,
                is_custom: true,
                members: custom_members(nodes, &custom.member_keywords),
            });
        }

        groups.push(GeneratedGroup {
            name: DIRECT_GROUP.to_owned(),
            group_type: "select".to_owned(),
            is_master: false,
            is_custom: false,
            members: vec!["DIRECT".to_owned()],
        });
        groups.push(GeneratedGroup {
            name: REJECT_GROUP.to_owned(),
            group_type: "select".to_owned(),
            is_master: false,
            is_custom: false,
            members: vec!["REJECT".to_owned(), "DIRECT".to_owned()],
        });

        groups
    }

    /// Render the plan as a standalone mihomo YAML document.
    fn render_yaml(nodes: &[ProxyNodeItem], groups: &[GeneratedGroup]) -> Result<String> {
        let mut doc = serde_yaml_ng::Mapping::new();
        doc.insert(
            serde_yaml_ng::Value::String("port".into()),
            serde_yaml_ng::Value::Number(7890.into()),
        );
        doc.insert(
            serde_yaml_ng::Value::String("socks-port".into()),
            serde_yaml_ng::Value::Number(7891.into()),
        );
        doc.insert(
            serde_yaml_ng::Value::String("mode".into()),
            serde_yaml_ng::Value::String("rule".into()),
        );
        doc.insert(
            serde_yaml_ng::Value::String("log-level".into()),
            serde_yaml_ng::Value::String("info".into()),
        );
        doc.insert(
            serde_yaml_ng::Value::String("proxies".into()),
            serde_yaml_ng::to_value(nodes)?,
        );

        if !groups.is_empty() {
            let group_values: Vec<serde_yaml_ng::Value> = groups
                .iter()
                .map(|group| {
                    let mut entry = serde_yaml_ng::Mapping::new();
                    entry.insert(
                        serde_yaml_ng::Value::String("name".into()),
                        serde_yaml_ng::Value::String(group.name.clone()),
                    );
                    entry.insert(
                        serde_yaml_ng::Value::String("type".into()),
                        serde_yaml_ng::Value::String(group.group_type.clone()),
                    );
                    if group.group_type == "url-test" {
                        entry.insert(
                            serde_yaml_ng::Value::String("url".into()),
                            serde_yaml_ng::Value::String(AGGREGATION_HEALTH_CHECK_URL.into()),
                        );
                        entry.insert(
                            serde_yaml_ng::Value::String("interval".into()),
                            serde_yaml_ng::Value::Number(AGGREGATION_HEALTH_CHECK_INTERVAL.into()),
                        );
                        entry.insert(
                            serde_yaml_ng::Value::String("tolerance".into()),
                            serde_yaml_ng::Value::Number(AGGREGATION_HEALTH_CHECK_TOLERANCE.into()),
                        );
                    }
                    entry.insert(
                        serde_yaml_ng::Value::String("proxies".into()),
                        serde_yaml_ng::to_value(&group.members)?,
                    );
                    Ok(serde_yaml_ng::Value::Mapping(entry))
                })
                .collect::<Result<Vec<_>>>()?;
            doc.insert(
                serde_yaml_ng::Value::String("proxy-groups".into()),
                serde_yaml_ng::Value::Sequence(group_values),
            );
        }

        let rules = vec![serde_yaml_ng::Value::String(format!(
            "MATCH,{MASTER_SELECT_GROUP}"
        ))];
        doc.insert(
            serde_yaml_ng::Value::String("rules".into()),
            serde_yaml_ng::Value::Sequence(rules),
        );

        serde_yaml_ng::to_string(&serde_yaml_ng::Value::Mapping(doc)).map_err(|e| anyhow!("{e}"))
    }
}

/// DUAL-08-10: member selection for a user-authored group. Empty keywords
/// select every surviving node; otherwise a node joins when its name contains
/// any keyword (case-insensitive).
fn custom_members(nodes: &[ProxyNodeItem], keywords: &[String]) -> Vec<String> {
    let needles: Vec<String> = keywords
        .iter()
        .map(|keyword| keyword.trim().to_lowercase())
        .filter(|keyword| !keyword.is_empty())
        .collect();
    if needles.is_empty() {
        return nodes.iter().map(|node| node.name.clone()).collect();
    }
    nodes
        .iter()
        .filter(|node| {
            let lower = node.name.to_lowercase();
            needles.iter().any(|needle| lower.contains(needle))
        })
        .map(|node| node.name.clone())
        .collect()
}

/// Human label for an ISO region, sourced from the shared [`COUNTRY_DEFS`]
/// alias table so the UI never invents a name the matcher would not accept.
pub fn region_label(iso: &str) -> String {
    for (code, aliases) in crate::filter::COUNTRY_DEFS {
        if code.eq_ignore_ascii_case(iso)
            && let Some(label) = aliases.iter().find(|alias| contains_cjk(alias))
        {
            return (*label).to_owned();
        }
    }
    iso.to_ascii_uppercase()
}

/// Whether `text` contains at least one CJK ideograph. Used to pick the native
/// label out of the bilingual [`COUNTRY_DEFS`] alias table.
pub fn contains_cjk(text: &str) -> bool {
    text.chars().any(|ch| {
        let code = ch as u32;
        (0x4E00..=0x9FFF).contains(&code) || (0x3400..=0x4DBF).contains(&code)
    })
}

/// Regional-indicator flag derived from an ISO alpha-2 code.
pub fn region_flag(iso: &str) -> String {
    let mut flag = String::new();
    for ch in iso.chars().take(2) {
        let upper = ch.to_ascii_uppercase();
        if !upper.is_ascii_alphabetic() {
            continue;
        }
        let base = 0x1F1E6u32;
        let offset = u32::from(upper as u8 - b'A');
        if let Some(symbol) = char::from_u32(base + offset) {
            flag.push(symbol);
        }
    }
    flag
}
