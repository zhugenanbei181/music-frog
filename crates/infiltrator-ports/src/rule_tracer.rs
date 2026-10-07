//! Profile source and atomic rule persistence capabilities for the shared command owner.
//!
//! The shared `RuleTracerApplication` engine lives in `infiltrator-application`;
//! UIs submit commands and read snapshots. They cannot receive a mutable
//! tracer engine through a host runtime facade.

use crate::error::PortError;
use async_trait::async_trait;
use infiltrator_contract::error::Failure;
use infiltrator_contract::rule_location::RuleLocation;
use infiltrator_contract::rule_source::RuleSourceIdentity;
use infiltrator_domain::rules::RuleEntry;
use std::collections::BTreeMap;

pub struct RuleWorkspace {
    pub source: RuleSourceIdentity,
    pub rules: Vec<RuleEntry>,
    pub targets: Vec<String>,
    pub sub_rules: BTreeMap<String, Vec<RuleEntry>>,
}

/// DUAL-12-08: the persistence capability behind the tracer reverse-apply.
/// A host that can rewrite the live rule list and commit it through the
/// existing apply transaction implements this port; `RuleTracerApplication`
/// owns the rule-rewrite logic and delegates only the load/apply legs.
#[async_trait]
pub trait RuleOverridePort: Send + Sync {
    /// Load the current profile's rule list, in file order.
    async fn load_rule_workspace(&self) -> Result<RuleWorkspace, PortError>;

    /// Persist a complete rewritten rule list through the atomic apply
    /// transaction (validate, temp write, reload/restart, readiness, rollback).
    async fn compare_and_apply_rules(
        &self,
        expected: &RuleWorkspace,
        entries: &[RuleEntry],
    ) -> Result<(), PortError>;

    async fn compare_and_apply_named_rule(
        &self,
        expected: &RuleWorkspace,
        location: &RuleLocation,
        expected_rule: &str,
        replacement: &str,
    ) -> Result<(), PortError> {
        let _ = (expected, location, expected_rule, replacement);
        Err(PortError::Rejected(Failure::unsupported(
            "Qualified sub-rule persistence is not composed",
        )))
    }
}
