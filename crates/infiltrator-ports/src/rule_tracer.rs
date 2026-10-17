//! Profile source and atomic rule persistence capabilities for the shared command owner.
//!
//! The shared `RuleTracerApplication` engine lives in `infiltrator-application`;
//! UIs submit commands and read snapshots. They cannot receive a mutable
//! tracer engine through a host runtime facade.

use crate::error::PortError;
use async_trait::async_trait;
use infiltrator_contract::profile_document_identity::ProfileWorkspaceIdentity;
use infiltrator_contract::rule_location::RuleLocation;
use infiltrator_contract::rule_source::RuleSourceIdentity;
use infiltrator_contract::rule_trace_run::RuleTraceOwnerBinding;
use infiltrator_domain::rules::RuleEntry;
use lurker_core_contract::error::{ErrorCode, Failure};
use std::collections::BTreeMap;

pub struct RuleWorkspace {
    pub source: RuleSourceIdentity,
    pub profile_identity: ProfileWorkspaceIdentity,
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

    /// Load rules only for the exact Core activation and typed profile
    /// workspace already admitted by the application. Implementations should
    /// recheck the active owner and workspace at their read boundary.
    async fn load_rule_workspace_for_trace(
        &self,
        owner: &RuleTraceOwnerBinding,
    ) -> Result<RuleWorkspace, PortError> {
        if owner.workspace.document.engine_id != owner.scope.identity.engine {
            return Err(PortError::Rejected(Failure::unsupported(
                "rule trace profile belongs to another engine",
            )));
        }
        let workspace = self.load_rule_workspace().await?;
        if workspace.profile_identity != owner.workspace {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "Profile workspace changed; refresh before simulating rules",
                true,
            )));
        }
        Ok(workspace)
    }

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
