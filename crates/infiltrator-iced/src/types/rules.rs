//! Rules-domain types: the rules load bundle, page tabs, tracer state,
//! and the rendered rule rows consumed by the rules view.

use infiltrator_contract::error::Failure;
use infiltrator_domain::rules::tracer::RuleTraceMatch;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RuleBadgeKind {
    Domain,
    Ip,
    #[default]
    Other,
}

#[derive(Debug, Clone, Default)]
pub struct RuleRenderItem {
    pub hit_count: Option<u64>,
    pub is_shadowed: bool,
    pub source_ip: bool,
    pub no_resolve: bool,
    pub failure: Option<Failure>,
    pub source_index: usize,
    pub rule_type: String,
    pub payload: String,
    pub target: String,
    pub badge: RuleBadgeKind,
}

/// State for the interactive Live Rule Tracer sandbox.
#[derive(Debug, Clone, Default)]
pub struct RuleTracerState {
    pub query: String,
    pub port_input: String,
    pub process_input: String,
    pub in_type_input: String,
    pub match_result: Option<RuleTraceMatch>,
    pub trace_performed: bool,
}

/// State for Rule-Provider unpacking and local cache purging.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProviderUnpackState {
    pub unpacked_rules_count: usize,
    pub is_purging_cache: bool,
    /// DUAL-11-06: a provider read is in flight.
    pub is_unpacking: bool,
    /// Last honest outcome ("provider · N rules · origin …" or the typed
    /// failure reason). Never a fabricated success string.
    pub status_message: Option<String>,
}
