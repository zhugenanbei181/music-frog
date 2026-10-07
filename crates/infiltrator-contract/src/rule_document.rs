//! Complete editable source facts, independent of a rendered rule window.
use crate::rule_source::RuleSourceIdentity;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleDefinition {
    pub rule: String,
    pub enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleDocumentSnapshot {
    pub source: RuleSourceIdentity,
    pub rules: Vec<RuleDefinition>,
    pub sub_rules: BTreeMap<String, Vec<RuleDefinition>>,
    pub targets: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleListCommit {
    pub expected_source: RuleSourceIdentity,
    pub rules: Vec<RuleDefinition>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleListOperationId(pub u64);

/// One staged row, distinct even when two rule expressions are identical.
/// Identities are retired when a draft is discarded or a new source is adopted.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RuleRowId(pub u64);
