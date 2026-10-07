//! Typed condition evaluation, including uncertainty; unknown never becomes false.
use crate::rule_location::RuleLocation;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum TrafficField {
    Destination,
    DestinationIp,
    DestinationPort,
    SourceIp,
    SourcePort,
    InboundPort,
    InboundType,
    InboundName,
    InboundUser,
    ProcessName,
    ProcessPath,
    Network,
    Dscp,
    Uid,
    PackageName,
}
impl TrafficField {
    pub const SANDBOX: [Self; 13] = [
        Self::DestinationIp,
        Self::DestinationPort,
        Self::SourcePort,
        Self::InboundPort,
        Self::InboundType,
        Self::InboundName,
        Self::InboundUser,
        Self::ProcessName,
        Self::ProcessPath,
        Self::Network,
        Self::Dscp,
        Self::Uid,
        Self::PackageName,
    ];
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConditionIssue {
    MissingInput(TrafficField),
    ExternalData { rule_type: String, name: String },
    InvalidRule { reason: String },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConditionOutcome {
    Matched,
    NotMatched,
    Unresolved(ConditionIssue),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvaluationNodeKind {
    Leaf,
    And,
    Or,
    Not,
    SubRule,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleEvaluationNode {
    pub depth: usize,
    pub kind: EvaluationNodeKind,
    pub expression: Option<String>,
    pub outcome: ConditionOutcome,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleTraceIssue {
    pub index: usize,
    #[serde(default)]
    pub location: Option<RuleLocation>,
    pub rule: String,
    pub issue: ConditionIssue,
}
