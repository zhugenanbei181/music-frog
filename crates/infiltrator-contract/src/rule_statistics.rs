//! Actual local-statistics reset boundary, independent of any UI or runtime.
use crate::error::{ErrorCode, Failure};
use crate::rule_hit_audit::{RuleHitAuditSnapshot, RuleHitSummary};
use crate::rule_source::RuleSourceIdentity;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Source-bound rule hit facts returned by the application owner.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RuleStatisticsObservation {
    pub audit: RuleHitAuditSnapshot,
    #[serde(default)]
    pub rows: BTreeMap<usize, RuleHitSummary>,
}

impl RuleStatisticsObservation {
    pub fn row_count(&self, index: usize) -> u64 {
        self.rows.get(&index).map_or(0, |row| row.hit_count)
    }

    pub fn row_timestamp(&self, index: usize) -> Option<u64> {
        self.rows.get(&index).and_then(|row| row.last_hit_secs)
    }
}

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
