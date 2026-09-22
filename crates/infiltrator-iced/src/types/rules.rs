//! Rules-domain types: the rules load bundle, page tabs, tracer state,
//! and the rendered rule rows consumed by the rules view.

use infiltrator_domain::rules::RuleEntry;
use infiltrator_domain::rules::tracer::RuleTraceMatch;

#[derive(Debug, Clone, Default)]
pub struct RulesLoadBundle {
    pub rules: Vec<RuleEntry>,
    pub rule_providers_json: String,
    pub proxy_providers_json: String,
    pub sniffer_json: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RuleBadgeKind {
    Domain,
    Ip,
    #[default]
    Other,
}

#[derive(Debug, Clone, Default)]
pub struct RuleRenderItem {
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

/// State for the Rule Hit Counter and Stale Rule Analyzer.
///
/// `audit` is the shared application-owned read model; `zero_hit_rule_indices`
/// is a pure projection of it onto the locally loaded rule list used by the
/// one-click disable action. No UI-local hit counts are fabricated.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RuleHitAuditState {
    pub audit: infiltrator_contract::rule_tracer::RuleHitAuditSnapshot,
    pub zero_hit_rule_indices: Vec<usize>,
    pub is_auditing: bool,
    pub audit_summary: Option<String>,
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
