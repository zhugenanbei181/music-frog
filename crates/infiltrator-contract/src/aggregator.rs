//! DUAL-08 shared contract for the multi-subscription aggregator.
//!
//! The domain owns the cleaning/clustering/group-synthesis algorithm
//! (`infiltrator_domain::profile_aggregator`); this module is the language
//! neutral, serializable read model shared by the application, the surface
//! reader, and both UI surfaces. No surface re-implements clustering or
//! group synthesis.

use serde::{Deserialize, Serialize};

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
}

impl AggregationDraft {
    /// Whether the draft can produce a preview: at least one source and the
    /// topology switch on.
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
