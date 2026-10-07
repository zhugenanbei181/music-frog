//! Qualified source locations distinguish root rules from named rule tables.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleLocation {
    pub table: Option<String>,
    pub index: usize,
}
impl RuleLocation {
    pub fn root(index: usize) -> Self {
        Self { table: None, index }
    }
    pub fn named(table: String, index: usize) -> Self {
        Self {
            table: Some(table),
            index,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RulePathEntry {
    pub location: RuleLocation,
    pub raw: String,
}
