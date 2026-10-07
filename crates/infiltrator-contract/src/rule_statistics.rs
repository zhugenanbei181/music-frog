//! Actual local-statistics reset boundary, independent of any UI or runtime.
use crate::error::{ErrorCode, Failure};
use crate::rule_source::RuleSourceIdentity;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleStatisticsResetReceipt {
    pub source: RuleSourceIdentity,
    pub revision: u64,
    pub removed_hits: u64,
    pub removed_rows: usize,
}
impl RuleStatisticsResetReceipt {
    pub fn validate(&self, expected: &RuleSourceIdentity) -> Result<(), Failure> {
        if &self.source != expected
            || self.revision == 0
            || self.removed_rows as u64 > self.removed_hits
        {
            Err(Failure::new(
                ErrorCode::InvalidState,
                "Statistics reset returned an invalid source or receipt",
                false,
            ))
        } else {
            Ok(())
        }
    }
}
