//! Source-bound local simulation observations and qualified row diagnoses.
use crate::rule_source::RuleSourceIdentity;
use serde::{Deserialize, Serialize};

/// Why a rule is considered dead / non-contributing during hit audit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleDeadReason {
    /// The rule has no successful local simulation hits in the observed source scope.
    ZeroHits,
    /// The rule is unreachable because an earlier rule shadows it.
    Shadowed,
}

/// Aggregated local trace statistics for a single root rule.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleHitSummary {
    #[serde(default)]
    pub rule_index: Option<usize>,
    pub rule_raw: String,
    pub hit_count: u64,
    pub total_payload_bytes: Option<u64>,
    pub last_hit_secs: Option<u64>,
}

/// A rule flagged as non-contributing, with the honest reason.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleDeadEntry {
    #[serde(default)]
    pub rule_index: Option<usize>,
    pub rule_raw: String,
    pub hit_count: u64,
    pub reason: RuleDeadReason,
    pub shadowed_by: Option<String>,
    pub detail: Option<String>,
    pub last_hit_secs: Option<u64>,
}

/// Shared read model for rule hit counting, dead-rule diagnosis and CIDR
/// conflict auditing. Computed once in the application and consumed by both
/// Iced and Bevy so neither surface keeps a private hit fact source.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RuleHitAuditSnapshot {
    #[serde(default)]
    pub revision: u64,
    #[serde(default)]
    pub source: Option<RuleSourceIdentity>,
    pub total_hits: u64,
    pub tracked_rules: usize,
    pub top_hits: Vec<RuleHitSummary>,
    /// Zero-hit and shadowed rules, ordered by rule order.
    pub dead_rules: Vec<RuleDeadEntry>,
    /// Subset of `dead_rules` whose cause is an overlapping IP-CIDR mask.
    pub cidr_overlaps: Vec<RuleDeadEntry>,
    /// Most recently hit rule (used for the hit-flash highlight).
    pub last_hit_rule: Option<String>,
    pub last_hit_secs: Option<u64>,
    /// Whether a counter reset is meaningful right now.
    pub can_clear: bool,
    /// Number of simulations recorded for the latency-contribution audit.
    pub trace_count: u64,
    /// Mean AST match latency across recorded simulations, in microseconds.
    pub avg_match_latency_us: Option<f64>,
    /// AST match latency of the most recent simulation, in microseconds.
    pub last_match_latency_us: Option<u64>,
}

impl RuleHitAuditSnapshot {
    pub fn demo_fixture() -> Self {
        Self {
            source: None,
            revision: 1,
            total_hits: 1287,
            tracked_rules: 42,
            top_hits: vec![
                RuleHitSummary {
                    rule_index: None,
                    rule_raw: "DOMAIN-SUFFIX,github.com,PROXY".to_owned(),
                    hit_count: 312,
                    total_payload_bytes: Some(48_233_984),
                    last_hit_secs: Some(1_700_000_012),
                },
                RuleHitSummary {
                    rule_index: None,
                    rule_raw: "DOMAIN-KEYWORD,bilibili,DIRECT".to_owned(),
                    hit_count: 186,
                    total_payload_bytes: Some(22_114_560),
                    last_hit_secs: Some(1_700_000_004),
                },
            ],
            dead_rules: vec![
                RuleDeadEntry {
                    rule_index: None,
                    rule_raw: "DOMAIN,dead.example.com,REJECT".to_owned(),
                    hit_count: 0,
                    reason: RuleDeadReason::ZeroHits,
                    shadowed_by: None,
                    detail: None,
                    last_hit_secs: None,
                },
                RuleDeadEntry {
                    rule_index: None,
                    rule_raw: "IP-CIDR,10.1.2.0/24,PROXY".to_owned(),
                    hit_count: 0,
                    reason: RuleDeadReason::Shadowed,
                    shadowed_by: Some("IP-CIDR,10.0.0.0/8,DIRECT".to_owned()),
                    detail: Some(
                        "IP CIDR is shadowed by an earlier broader IP-CIDR rule".to_owned(),
                    ),
                    last_hit_secs: None,
                },
            ],
            cidr_overlaps: vec![RuleDeadEntry {
                rule_index: None,
                rule_raw: "IP-CIDR,10.1.2.0/24,PROXY".to_owned(),
                hit_count: 0,
                reason: RuleDeadReason::Shadowed,
                shadowed_by: Some("IP-CIDR,10.0.0.0/8,DIRECT".to_owned()),
                detail: Some("IP CIDR is shadowed by an earlier broader IP-CIDR rule".to_owned()),
                last_hit_secs: None,
            }],
            last_hit_rule: Some("DOMAIN-SUFFIX,github.com,PROXY".to_owned()),
            last_hit_secs: Some(1_700_000_012),
            can_clear: true,
            trace_count: 128,
            avg_match_latency_us: Some(18.5),
            last_match_latency_us: Some(14),
        }
    }
}
