//! Typed facts for simulation stages; no inferred value claims a packet observation.
use crate::rule_condition::RuleEvaluationNode;
use crate::rule_location::RulePathEntry;
use crate::rule_tracer::TrafficContextSnapshot;
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DecisionStageFacts {
    Inbound(TrafficContextSnapshot),
    Sniffer {
        domain: Option<String>,
        ip: Option<String>,
        port: Option<u16>,
    },
    Rule {
        index: Option<usize>,
        raw: String,
        target: String,
        no_resolve: Option<bool>,
        evaluations: Vec<RuleEvaluationNode>,
        #[serde(default)]
        path: Vec<RulePathEntry>,
    },
    Policy {
        target: String,
        selected: Option<String>,
    },
    Outbound {
        name: Option<String>,
        protocol: Option<String>,
        delay_ms: Option<u32>,
        country: Option<String>,
    },
}
