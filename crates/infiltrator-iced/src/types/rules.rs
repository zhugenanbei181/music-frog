//! Rules-domain types: the rules load bundle, page tabs, tracer state,
//! and the rendered rule rows consumed by the rules view.

use infiltrator_core::rules::RuleEntry;
use infiltrator_core::rules::tracer::RuleTraceMatch;

#[derive(Debug, Clone, Default)]
pub struct RulesLoadBundle {
    pub rules: Vec<RuleEntry>,
    pub rule_providers_json: String,
    pub proxy_providers_json: String,
    pub sniffer_json: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RulesTab {
    #[default]
    RulesList,
    Providers,
    JsonEditors,
    Tracer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RulesJsonTab {
    #[default]
    RuleProviders,
    ProxyProviders,
    Sniffer,
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
    pub search_key: String,
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

/// Draft state for the visual Sub-Rules and logical rule AST builder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubRuleDraft {
    pub operator: String,
    pub conditions: Vec<String>,
    pub target: String,
}

impl Default for SubRuleDraft {
    fn default() -> Self {
        Self {
            operator: "AND".to_string(),
            conditions: vec![
                "DOMAIN-SUFFIX,company.com".to_string(),
                "NETWORK,TCP".to_string(),
            ],
            target: "DIRECT".to_string(),
        }
    }
}

/// State for the Rule Hit Counter and Stale Rule Analyzer.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuleHitAuditState {
    pub total_rule_hits: usize,
    pub zero_hit_rule_indices: Vec<usize>,
    pub is_auditing: bool,
    pub audit_summary: Option<String>,
}

/// State for Rule-Provider unpacking and local cache purging.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProviderUnpackState {
    pub unpacked_rules_count: usize,
    pub is_purging_cache: bool,
    pub status_message: Option<String>,
}
