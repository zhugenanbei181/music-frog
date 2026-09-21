//! Host-owned live rule tracer seam.
//!
//! The shared `RuleTracerApplication` engine lives in `infiltrator-application`;
//! this port is the zero-toolkit seam an inbound UI uses to drive the same
//! instance the surface reader projects, so both surfaces observe one query
//! state and one decision-chain computation instead of UI-local copies.

use infiltrator_contract::active_exit::ActiveExitSnapshot;
use infiltrator_contract::rule_tracer::DecisionChainSnapshot;
use infiltrator_domain::rules::RuleEntry;

pub trait RuleTracerPort: Send + Sync {
    /// Record the query that the next surface projection should replay.
    fn set_query(&self, query: &str);

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
}
