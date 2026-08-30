//! Rules-domain types: the rules load bundle, page tabs and the rendered
//! rule rows consumed by the rules view.

use infiltrator_core::rules::RuleEntry;

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
