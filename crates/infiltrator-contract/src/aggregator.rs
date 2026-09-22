//! DUAL-08 shared contract for the multi-subscription aggregator.
//!
//! The domain owns the cleaning/clustering/group-synthesis algorithm
//! (`infiltrator_domain::profile_aggregator`); this module is the language
//! neutral, serializable read model shared by the application, the surface
//! reader, and both UI surfaces. No surface re-implements clustering or
//! group synthesis.

use serde::{Deserialize, Serialize};

/// DUAL-08-08: one user-authored regular-expression rename rule applied to
/// node names during aggregation.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AggregationRenameRule {
    /// Regular expression matched against the node name.
    pub pattern: String,
    /// Replacement text (supports capture groups).
    pub replacement: String,
}

impl AggregationRenameRule {
    /// Parse the shared free-text syntax used by both surfaces: one
    /// `pattern => replacement` rule per line or per `;`. Returns the typed
    /// rules, or the offending line as a validation message so surfaces can
    /// point at the exact rule that failed.
    pub fn parse_list(text: &str) -> Result<Vec<Self>, String> {
        let mut rules = Vec::new();
        for line in text.split(['\n', ';']) {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let Some((pattern, replacement)) = line.split_once("=>") else {
                return Err(line.to_string());
            };
            let pattern = pattern.trim();
            if pattern.is_empty() {
                return Err(line.to_string());
            }
            rules.push(Self {
                pattern: pattern.to_string(),
                replacement: replacement.trim().to_string(),
            });
        }
        Ok(rules)
    }

    /// Render rules back into the shared free-text syntax (one per line).
    pub fn to_text(rules: &[Self]) -> String {
        rules
            .iter()
            .map(|rule| format!("{} => {}", rule.pattern, rule.replacement))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// DUAL-08-10: one user-authored proxy group appended to the synthesized
/// topology. Members are selected by node-name keywords; no keywords selects
/// every surviving node.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AggregationCustomGroup {
    /// Group name written into the profile.
    pub name: String,
    /// Mihomo group type: `select` or `url-test`.
    pub group_type: String,
    /// Case-insensitive node-name keywords selecting the members.
    pub member_keywords: Vec<String>,
}

/// DUAL-08-13: a persisted aggregation draft that can be reused later.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AggregationTemplate {
    /// User-chosen template label.
    pub name: String,
    /// The remembered wizard configuration.
    pub draft: AggregationDraft,
    /// RFC3339 instant the template was last saved.
    pub updated_at: String,
}

/// The persisted aggregation selection/options template (DUAL-08-01).
///
/// The draft is the single user-owned input: which source profiles are
/// selected, the target name, and the cleaning/topology switches. Both
/// surfaces edit it and both consume the report derived from it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AggregationDraft {
    /// Selected source profile names, in report order.
    pub source_profiles: Vec<String>,
    /// Name for the profile a merge would produce.
    pub target_name: String,
    /// DUAL-08-02: drop fingerprint-identical nodes (server+port+protocol+
    /// credentials) across the selected sources.
    pub deduplicate: bool,
    /// DUAL-08-02: resolve remaining duplicate *names* by appending an index.
    pub deduplicate_names: bool,
    /// DUAL-08-03: normalise node names to `[ISO] name` so geo clustering
    /// can bucket them.
    pub geo_cluster: bool,
    /// DUAL-08-04/08-05: synthesize the regional `url-test` groups and the
    /// master selector that cascades them.
    pub generate_groups: bool,
    /// Strip emoji characters from node names.
    pub remove_emojis: bool,
    /// DUAL-08-08: regex rename rules applied before cleaning/grouping.
    pub rename_rules: Vec<AggregationRenameRule>,
    /// DUAL-08-10: user-authored groups appended to the synthesized cascade.
    pub custom_groups: Vec<AggregationCustomGroup>,
    /// DUAL-08-09: drop nodes failing the required-field precheck.
    pub availability_precheck: bool,
    /// DUAL-08-12: make the generated profile the active profile (and hot
    /// reload the kernel when the host exposes a managed runtime seam).
    pub activate_after_create: bool,
}

impl AggregationDraft {
    /// Whether the draft can produce a preview: at least one source profile
    /// selected. Every cleaning/topology switch is additive.
    pub fn is_previewable(&self) -> bool {
        !self.source_profiles.is_empty()
    }
}

/// One ISO region cluster in the report (DUAL-08-03).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RegionalClusterSnapshot {
    /// ISO-3166 alpha-2 code, e.g. `HK`.
    pub iso: String,
    /// Native region label, e.g. `香港`.
    pub label: String,
    /// Regional-indicator flag, e.g. `🇭🇰`.
    pub flag: String,
    /// Generated `url-test` group name, e.g. `香港自动测速`.
    pub group_name: String,
    /// Node names in this cluster.
    pub node_names: Vec<String>,
}

/// One synthesized proxy group in the cascade topology (DUAL-08-04/08-05).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GeneratedGroupSnapshot {
    pub name: String,
    pub group_type: String,
    /// DUAL-08-05: whether this is the master selector cascading the regions.
    pub is_master: bool,
    /// DUAL-08-10: whether this group came from the user-authored topology.
    pub is_custom: bool,
    pub members: Vec<String>,
}

/// The shared aggregation preview report.
///
/// Produced from a real [`AggregationDraft`] over real profile contents; the
/// counters are the pipeline's own counters, never estimates.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AggregationReport {
    /// The draft this report was produced from.
    pub draft: AggregationDraft,
    /// Sources whose content contributed at least one node.
    pub source_count: usize,
    /// Selected sources that could not be read or parsed.
    pub missing_sources: Vec<String>,
    /// Node count before cleaning.
    pub input_nodes: usize,
    /// Node count after cleaning.
    pub total_nodes: usize,
    /// Nodes removed by fingerprint/name deduplication.
    pub duplicates_removed: usize,
    /// Nodes renamed by country-code normalisation.
    pub renamed_nodes: usize,
    /// DUAL-08-08: nodes renamed by the user's regex rules.
    pub rule_renamed_nodes: usize,
    /// DUAL-08-09: nodes dropped by the required-field precheck.
    pub invalid_nodes_removed: usize,
    /// DUAL-08-09: bounded sample of the precheck findings, one per dropped
    /// node (name plus the first reported problem).
    pub invalid_node_samples: Vec<String>,
    /// Region clusters derived from the surviving nodes.
    pub regions: Vec<RegionalClusterSnapshot>,
    /// Synthesized cascade groups (empty when topology generation is off).
    pub groups: Vec<GeneratedGroupSnapshot>,
    /// Full rendered mihomo YAML document.
    pub yaml: String,
    /// RFC3339 instant the report was produced.
    pub generated_at: String,
}

impl AggregationReport {
    /// A bounded YAML viewport for the surface preview (DUAL-08-11).
    pub fn yaml_preview(&self, max_lines: usize) -> String {
        let mut lines = self.yaml.lines();
        let mut preview: Vec<&str> = lines.by_ref().take(max_lines).collect();
        let truncated = lines.next().is_some();
        if truncated {
            preview.push("...");
        }
        preview.join("\n")
    }

    /// The master selector group when topology generation ran.
    pub fn master_group(&self) -> Option<&GeneratedGroupSnapshot> {
        self.groups.iter().find(|group| group.is_master)
    }
}

/// DUAL-08-12: outcome of materialising a draft into a profile.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AggregatedProfileOutcome {
    /// Sanitized name the profile was saved under.
    pub profile_name: String,
    /// The report the profile was rendered from.
    pub report: AggregationReport,
    /// Whether the profile is now the active profile. When activation was
    /// requested this is always true (a failed switch is an error, not a
    /// silent partial success).
    pub activated: bool,
    /// Whether a running kernel accepted the re-applied configuration. A host
    /// without a managed-runtime seam honestly reports `false` here while
    /// `activated` stays true, exactly like the shared `SwitchProfile` path.
    pub core_reloaded: bool,
    /// DUAL-08-13: the template the draft was remembered under, `None` when
    /// the host store keeps no template sidecar (typed unsupported).
    #[serde(default)]
    pub template_name: Option<String>,
}
