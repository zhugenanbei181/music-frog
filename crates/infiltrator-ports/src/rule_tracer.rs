//! Host-owned live rule tracer seam.
//!
//! The shared `RuleTracerApplication` engine lives in `infiltrator-application`;
//! this port is the zero-toolkit seam an inbound UI uses to drive the same
//! instance the surface reader projects, so both surfaces observe one query
//! state and one decision-chain computation instead of UI-local copies.

use async_trait::async_trait;
use infiltrator_contract::active_exit::ActiveExitSnapshot;
use infiltrator_contract::rule_tracer::{
    DecisionChainSnapshot, TracerRuleOverride, TracerRuleOverrideResult, TrafficContextSnapshot,
};
use infiltrator_domain::rules::RuleEntry;

#[async_trait]
pub trait RuleTracerPort: Send + Sync {
    /// Record the query that the next surface projection should replay.
    fn set_query(&self, query: &str);

    /// DUAL-12-10: record the simulated inbound sandbox context (source IP /
    /// source port / inbound port) merged onto every subsequent trace and
    /// projection so both surfaces observe one environment.
    fn set_context(&self, context: &TrafficContextSnapshot);

    /// Pure AST simulation of the routing decision chain for `query` over
    /// `rules`. No controller, filesystem, or network access.
    fn trace(
        &self,
        rules: &[RuleEntry],
        query: &str,
        active_exit: Option<&ActiveExitSnapshot>,
    ) -> DecisionChainSnapshot;

    /// Feed observed rule hits from a live connection stream into the shared
    /// counter. `payload_bytes` is the connection's cumulative traffic.
    fn record_hits(&self, hits: &[(&str, u64)]);

    /// Reset every accumulated hit counter and timestamp.
    fn clear_hits(&self);

    /// DUAL-12-08: reverse-apply the matched rule's outbound through the
    /// host's atomic config transaction (rewrite + reload + rollback). The
    /// result is the one typed value both surfaces consume; a host without a
    /// composed apply capability returns `Unsupported`, never a silent no-op.
    async fn apply_override(&self, request: &TracerRuleOverride) -> TracerRuleOverrideResult;
}

/// DUAL-12-08: the persistence capability behind the tracer reverse-apply.
/// A host that can rewrite the live rule list and commit it through the
/// existing apply transaction implements this port; `RuleTracerApplication`
/// owns the rule-rewrite logic and delegates only the load/apply legs.
#[async_trait]
pub trait RuleOverridePort: Send + Sync {
    /// Load the current profile's rule list, in file order.
    async fn load_rule_entries(&self) -> Result<Vec<RuleEntry>, crate::error::PortError>;

    /// Persist a complete rewritten rule list through the atomic apply
    /// transaction (validate, temp write, reload/restart, readiness, rollback).
    async fn apply_rule_entries(
        &self,
        entries: &[RuleEntry],
    ) -> Result<(), crate::error::PortError>;
}
