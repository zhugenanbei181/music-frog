//! One rule row's source, semantic facts and explicit parse failure.
use crate::error::Failure;
use crate::rule_document::RuleRowId;
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RuleSnapshot {
    pub id: usize,
    #[serde(default)]
    pub edit_id: Option<RuleRowId>,
    pub rule_type: String,
    pub payload: String,
    pub proxy: String,
    /// Local trace observation; missing or source-mismatched facts remain unknown.
    #[serde(default)]
    pub hit_count: Option<u64>,
    #[serde(default)]
    pub raw: String,
    #[serde(default)]
    pub source_ip: bool,
    #[serde(default)]
    pub failure: Option<Failure>,
    #[serde(default)]
    pub is_enabled: bool,
    #[serde(default)]
    pub no_resolve: bool,
    #[serde(default)]
    pub last_hit_secs: Option<u64>,
    #[serde(default)]
    pub is_shadowed: bool,
    #[serde(default)]
    pub shadow_reason: Option<String>,
}
