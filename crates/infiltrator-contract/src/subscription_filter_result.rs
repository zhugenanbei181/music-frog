//! Detailed filtering facts and the actual source published by a commit.
use crate::profile_source::ProfileSourceIdentity;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilterReport {
    pub total_input: usize,
    pub passed: usize,
    pub excluded_by_blacklist: usize,
    pub excluded_by_whitelist: usize,
    pub excluded_by_type: usize,
    #[serde(default)]
    pub excluded_by_port: usize,
    #[serde(default)]
    pub excluded_by_server: usize,
    pub renamed: usize,
    pub deduplicated: usize,
    #[serde(default)]
    pub mutated: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubscriptionFilterApplied {
    pub source: ProfileSourceIdentity,
    pub report: FilterReport,
}
