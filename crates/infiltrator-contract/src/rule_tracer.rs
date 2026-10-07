//! Shared read model and decision tree replay contract for the Live Rule Tracer (交互式分流追踪器).

use crate::error::{ErrorCode, Failure};
use crate::rule_condition::{ConditionOutcome, EvaluationNodeKind, RuleEvaluationNode};
#[cfg(test)]
use crate::rule_hit_audit::{RuleDeadReason, RuleHitAuditSnapshot};
use crate::rule_location::RulePathEntry;
use crate::rule_source::RuleSourceIdentity;
use crate::rule_trace_facts::DecisionStageFacts;
use serde::{Deserialize, Serialize};

/// Status of the Live Rule Tracer engine.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleTracerStatus {
    #[default]
    Unknown,
    Ready,
    Empty,
    Unsupported,
    Failed,
}

/// The 5 core stages in a Mihomo routing decision chain:
/// Inbound -> Sniffer -> RuleSet / Rule -> Proxy Group -> Outbound
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionStageKind {
    Inbound,
    Sniffer,
    RuleSet,
    ProxyGroup,
    Outbound,
}

impl DecisionStageKind {
    pub const ALL: [Self; 5] = [
        Self::Inbound,
        Self::Sniffer,
        Self::RuleSet,
        Self::ProxyGroup,
        Self::Outbound,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Inbound => "inbound",
            Self::Sniffer => "sniffer",
            Self::RuleSet => "rule_set",
            Self::ProxyGroup => "proxy_group",
            Self::Outbound => "outbound",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Inbound => "入站监听 (Inbound)",
            Self::Sniffer => "协议嗅探 (Sniffer)",
            Self::RuleSet => "分流规则 (RuleSet / Rule)",
            Self::ProxyGroup => "策略组决策 (Proxy Group)",
            Self::Outbound => "出口节点 (Outbound)",
        }
    }
}

/// Evaluation outcome for an individual stage/node in the decision tree.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionNodeStatus {
    #[default]
    Neutral,
    Matched,
    Passed,
    Failed,
    Bypassed,
    Fallback,
}

/// A node in the hierarchical decision chain tree.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionChainNode {
    pub stage: DecisionStageKind,
    #[serde(default)]
    pub facts: Option<DecisionStageFacts>,
    pub title: String,
    pub detail: String,
    pub badge: Option<String>,
    pub status: DecisionNodeStatus,
    pub sub_evaluations: Vec<String>,
}

/// Detailed tree replay for the entire routing decision pipeline.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionChainSnapshot {
    pub nodes: Vec<DecisionChainNode>,
    pub hit_rule_index: Option<usize>,
    #[serde(default)]
    pub hit_rule_table: Option<String>,
    #[serde(default)]
    pub rule_path: Vec<RulePathEntry>,
    pub matched_rule_raw: String,
    pub matched_rule_type: String,
    pub matched_payload: String,
    pub target_proxy: String,
    pub final_outbound: String,
    pub final_node_protocol: Option<String>,
    pub final_node_delay_ms: Option<u32>,
    pub final_node_country: Option<String>,
    pub match_latency_us: u64,
    pub is_fallback: bool,
}

impl DecisionChainSnapshot {
    /// DUAL-12-08: a concrete, non-fallback rule matched, so its outbound can
    /// be rewritten by index. A fallback `MATCH` replay or a chain without a
    /// hit cannot be reverse-applied.
    pub fn can_reverse_apply(&self) -> bool {
        self.hit_rule_index.is_some() && !self.is_fallback
    }

    /// Suggested replacement outbound for the one-click override chooser. The
    /// suggestion flips away from the current target when doing so is
    /// meaningful; it is never a fabricated group name.
    pub fn suggested_override_target(&self) -> Option<String> {
        let current = self.target_proxy.trim();
        if current.is_empty() {
            None
        } else if current.eq_ignore_ascii_case("DIRECT") {
            Some("REJECT".to_owned())
        } else {
            Some("DIRECT".to_owned())
        }
    }
}

/// Normalized traffic context used for simulation queries.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrafficContextSnapshot {
    pub domain: Option<String>,
    pub ip: Option<String>,
    pub port: Option<u16>,
    /// Simulated inbound source IP (the LAN device the decision is replayed
    /// for). DUAL-12-10: this is the sandbox environment parameter that the
    /// Inbound decision stage reflects on both surfaces.
    #[serde(default)]
    pub src_ip: Option<String>,
    /// Simulated source port paired with `src_ip`.
    #[serde(default)]
    pub src_port: Option<u16>,
    /// Simulated inbound listener port the traffic arrives on.
    #[serde(default)]
    pub in_port: Option<u16>,
    pub process_name: Option<String>,
    pub process_path: Option<String>,
    pub in_name: Option<String>,
    pub in_user: Option<String>,
    pub dscp: Option<u8>,
    pub uid: Option<u32>,
    pub package_name: Option<String>,
    pub network: Option<String>,
    pub in_type: Option<String>,
    pub client_ip: Option<String>,
}

/// DUAL-12-08: one-click reverse-apply request. The traced decision matched
/// `rule_index`; the user picked `new_target` (PROXY / DIRECT / REJECT or a
/// typed group name) to rewrite that rule's outbound and re-apply the config.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TracerRuleOverride {
    pub rule_index: usize,
    #[serde(default)]
    pub rule_table: Option<String>,
    pub new_target: String,
    pub expected_source: RuleSourceIdentity,
    pub expected_rule: String,
}

/// Terminal status of a reverse-apply request. `Unsupported` is the honest
/// answer when no configuration/apply capability is composed; it is never a
/// silent success.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TracerRuleOverrideStatus {
    Applied,
    Unsupported,
    InvalidTarget,
    StaleRuleIndex,
    ApplyFailed,
}

/// The single typed result every surface consumes after a reverse-apply
/// attempt. Iced and Bevy both read this value; neither invents its own
/// success/failure, and the failure reason is always present when the rule
/// was not applied.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TracerRuleOverrideResult {
    pub status: TracerRuleOverrideStatus,
    pub rule_index: usize,
    pub new_target: String,
    pub previous_rule_raw: Option<String>,
    pub updated_rule_raw: Option<String>,
    pub failure: Option<String>,
    pub typed_failure: Option<Failure>,
}

impl TracerRuleOverrideResult {
    /// A committed override carrying both rule strings for the surface toast.
    pub fn applied(
        rule_index: usize,
        new_target: String,
        previous_rule_raw: String,
        updated_rule_raw: String,
    ) -> Self {
        Self {
            status: TracerRuleOverrideStatus::Applied,
            rule_index,
            new_target,
            previous_rule_raw: Some(previous_rule_raw),
            updated_rule_raw: Some(updated_rule_raw),
            failure: None,
            typed_failure: None,
        }
    }

    /// A rejected override. `failure` must explain the rejection.
    pub fn rejected(
        status: TracerRuleOverrideStatus,
        rule_index: usize,
        new_target: String,
        failure: impl Into<String>,
    ) -> Self {
        Self {
            status,
            rule_index,
            new_target,
            previous_rule_raw: None,
            updated_rule_raw: None,
            failure: Some(failure.into()),
            typed_failure: None,
        }
    }

    pub fn rejected_failure(
        status: TracerRuleOverrideStatus,
        request: &TracerRuleOverride,
        failure: Failure,
    ) -> Self {
        Self {
            status,
            rule_index: request.rule_index,
            new_target: request.new_target.clone(),
            previous_rule_raw: None,
            updated_rule_raw: None,
            failure: Some(failure.message.clone()),
            typed_failure: Some(failure),
        }
    }

    pub fn is_applied(&self) -> bool {
        self.status == TracerRuleOverrideStatus::Applied
    }

    pub fn failure_message(&self) -> Option<&str> {
        self.failure.as_deref()
    }

    /// Map a non-applied result onto the shared failure vocabulary; `None`
    /// means the override committed. The command router uses this so a
    /// rejected override is never reported as accepted.
    pub fn into_failure(self) -> Option<Failure> {
        if self.is_applied() {
            return None;
        }
        if self.typed_failure.is_some() {
            return self.typed_failure;
        }
        let message = self
            .failure
            .unwrap_or_else(|| "rule override was not applied".to_owned());
        let code = match self.status {
            TracerRuleOverrideStatus::Applied => ErrorCode::Internal,
            TracerRuleOverrideStatus::Unsupported => ErrorCode::Unsupported,
            TracerRuleOverrideStatus::InvalidTarget => ErrorCode::InvalidInput,
            TracerRuleOverrideStatus::StaleRuleIndex => ErrorCode::NotReady,
            TracerRuleOverrideStatus::ApplyFailed => ErrorCode::Configuration,
        };
        Some(Failure::new(code, message, false))
    }
}

/// Quick preset chip for 1-click test queries.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleTracerPreset {
    pub id: String,
    pub label: String,
    pub query: String,
    pub expected_target: Option<String>,
    pub description: String,
}

/// The comprehensive Rule Tracer sandbox read model.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RuleTracerSnapshot {
    pub generation: u64,
    pub revision: u64,
    pub status: RuleTracerStatus,
    pub failure: Option<String>,
    pub active_query: String,
    pub simulated_context: TrafficContextSnapshot,
    pub presets: Vec<RuleTracerPreset>,
    pub decision_chain: Option<DecisionChainSnapshot>,
    pub is_fallback_match: bool,
    pub total_rules_evaluated: usize,
    pub can_reverse_apply: bool,
    pub suggested_override_target: Option<String>,
    #[serde(default)]
    pub source: Option<RuleSourceIdentity>,
    #[serde(default)]
    pub targets: Vec<String>,
}

impl RuleTracerSnapshot {
    /// Standard preset chips recommended for interactive live simulation.
    pub fn default_presets() -> Vec<RuleTracerPreset> {
        vec![
            RuleTracerPreset {
                id: "google".to_owned(),
                label: "google.com".to_owned(),
                query: "google.com".to_owned(),
                expected_target: None,
                description: "国外主流搜索引擎 · 域名后缀匹配".to_owned(),
            },
            RuleTracerPreset {
                id: "github".to_owned(),
                label: "github.com".to_owned(),
                query: "github.com".to_owned(),
                expected_target: None,
                description: "开发者代码托管平台 · 域名后缀匹配".to_owned(),
            },
            RuleTracerPreset {
                id: "bilibili".to_owned(),
                label: "bilibili.com".to_owned(),
                query: "bilibili.com".to_owned(),
                expected_target: None,
                description: "国内多媒体网站 · GeoSite / 直连分流".to_owned(),
            },
            RuleTracerPreset {
                id: "cloudflare_dns".to_owned(),
                label: "1.1.1.1:443".to_owned(),
                query: "1.1.1.1:443".to_owned(),
                expected_target: None,
                description: "公共安全 DNS · IP-CIDR 掩码判定".to_owned(),
            },
            RuleTracerPreset {
                id: "steam".to_owned(),
                label: "steamcommunity.com".to_owned(),
                query: "steamcommunity.com".to_owned(),
                expected_target: None,
                description: "游戏社区服务 · 域名关键词分流".to_owned(),
            },
            RuleTracerPreset {
                id: "netflix".to_owned(),
                label: "netflix.com".to_owned(),
                query: "netflix.com".to_owned(),
                expected_target: None,
                description: "流媒体服务 · GeoIP / 媒体策略组".to_owned(),
            },
        ]
    }

    /// Explicit high-fidelity fixture for demo and headless test surfaces.
    pub fn demo_fixture() -> Self {
        let decision_chain = DecisionChainSnapshot {
            nodes: vec![
                DecisionChainNode {
                    stage: DecisionStageKind::Inbound,
                    facts: Some(DecisionStageFacts::Inbound(TrafficContextSnapshot {
                        src_ip: Some("127.0.0.1".into()),
                        src_port: Some(58421),
                        in_port: Some(7890),
                        network: Some("tcp".into()),
                        in_type: Some("mixed".into()),
                        ..TrafficContextSnapshot::default()
                    })),
                    title: "混合端口监听 (Mixed)".to_owned(),
                    detail: "127.0.0.1:7890 (TCP)".to_owned(),
                    badge: Some("IN-PORT 7890".to_owned()),
                    status: DecisionNodeStatus::Passed,
                    sub_evaluations: vec![
                        "客户端来源: 127.0.0.1:58421".to_owned(),
                        "入站协议: HTTP / SOCKS5 混合自适应".to_owned(),
                    ],
                },
                DecisionChainNode {
                    stage: DecisionStageKind::Sniffer,
                    facts: Some(DecisionStageFacts::Sniffer {
                        domain: Some("github.com".into()),
                        ip: None,
                        port: Some(443),
                    }),
                    title: "TLS SNI 域名嗅探".to_owned(),
                    detail: "嗅探提取目标: github.com (TLS 1.3)".to_owned(),
                    badge: Some("TLS-SNI".to_owned()),
                    status: DecisionNodeStatus::Passed,
                    sub_evaluations: vec![
                        "端口探测: 目标端口 443 触发 TLS 嗅探".to_owned(),
                        "嗅探结果: Host 与 SNI 一致 (github.com)".to_owned(),
                    ],
                },
                DecisionChainNode {
                    stage: DecisionStageKind::RuleSet,
                    facts: Some(DecisionStageFacts::Rule {
                        index: Some(41),
                        raw: "DOMAIN-SUFFIX,github.com,PROXY".into(),
                        target: "PROXY".into(),
                        no_resolve: Some(false),
                        path: Vec::new(),
                        evaluations: vec![RuleEvaluationNode {
                            depth: 0,
                            kind: EvaluationNodeKind::Leaf,
                            expression: Some("DOMAIN-SUFFIX,github.com".into()),
                            outcome: ConditionOutcome::Matched,
                        }],
                    }),
                    title: "命中规则 #42: DOMAIN-SUFFIX, github.com".to_owned(),
                    detail: "所属规则集: geosite-geolocation-!cn · 优先匹配命中".to_owned(),
                    badge: Some("DOMAIN-SUFFIX".to_owned()),
                    status: DecisionNodeStatus::Matched,
                    sub_evaluations: vec![
                        "[PASS] DOMAIN-SUFFIX 匹配: github.com".to_owned(),
                        "[PASS] no-resolve: 延迟域名解析".to_owned(),
                    ],
                },
                DecisionChainNode {
                    stage: DecisionStageKind::ProxyGroup,
                    facts: Some(DecisionStageFacts::Policy {
                        target: "PROXY".into(),
                        selected: Some("香港 IPLC 01".into()),
                    }),
                    title: "策略组决策: [PROXY]".to_owned(),
                    detail: "自动测速最低延迟选择 (url-test, 300s 间隔)".to_owned(),
                    badge: Some("url-test".to_owned()),
                    status: DecisionNodeStatus::Matched,
                    sub_evaluations: vec![
                        "策略组候选节点数: 16".to_owned(),
                        "最优节点筛选: 香港 IPLC 01 (28ms 延迟)".to_owned(),
                    ],
                },
                DecisionChainNode {
                    stage: DecisionStageKind::Outbound,
                    facts: Some(DecisionStageFacts::Outbound {
                        name: Some("香港 IPLC 01".into()),
                        protocol: Some("VLESS · Reality".into()),
                        delay_ms: Some(28),
                        country: Some("HK".into()),
                    }),
                    title: "最终出站: 香港 IPLC 01".to_owned(),
                    detail: "VLESS · Reality (延迟 28ms · 存活正常)".to_owned(),
                    badge: Some("HK".to_owned()),
                    status: DecisionNodeStatus::Matched,
                    sub_evaluations: vec![
                        "出口落地地区: 香港 (HK)".to_owned(),
                        "连接安全性: TLS 1.3 / Vision 隧道加密".to_owned(),
                    ],
                },
            ],
            hit_rule_table: None,
            rule_path: Vec::new(),
            hit_rule_index: Some(41), // 0-based
            matched_rule_raw: "DOMAIN-SUFFIX,github.com,PROXY".to_owned(),
            matched_rule_type: "DOMAIN-SUFFIX".to_owned(),
            matched_payload: "github.com".to_owned(),
            target_proxy: "PROXY".to_owned(),
            final_outbound: "香港 IPLC 01".to_owned(),
            final_node_protocol: Some("VLESS · Reality".to_owned()),
            final_node_delay_ms: Some(28),
            final_node_country: Some("HK".to_owned()),
            match_latency_us: 14,
            is_fallback: false,
        };

        Self {
            generation: 1,
            revision: 1,
            status: RuleTracerStatus::Ready,
            source: None,
            targets: Vec::new(),
            failure: None,
            active_query: "github.com".to_owned(),
            simulated_context: TrafficContextSnapshot {
                domain: Some("github.com".to_owned()),
                ip: None,
                port: Some(443),
                src_ip: Some("127.0.0.1".to_owned()),
                src_port: None,
                in_port: Some(7890),
                process_name: Some("git.exe".to_owned()),
                network: Some("tcp".to_owned()),
                in_type: Some("mixed".to_owned()),
                client_ip: Some("127.0.0.1".to_owned()),
                ..TrafficContextSnapshot::default()
            },
            presets: Self::default_presets(),
            decision_chain: Some(decision_chain),
            is_fallback_match: false,
            total_rules_evaluated: 42,
            can_reverse_apply: true,
            suggested_override_target: Some("DIRECT".to_owned()),
        }
    }

    pub fn ready(
        generation: u64,
        revision: u64,
        query: String,
        context: TrafficContextSnapshot,
        decision_chain: Option<DecisionChainSnapshot>,
        total_rules_evaluated: usize,
    ) -> Self {
        let is_fallback = decision_chain
            .as_ref()
            .map(|d| d.is_fallback)
            .unwrap_or(false);
        // DUAL-12-08: the two reverse-apply facts are derived from the shared
        // decision chain, never toggled independently by a surface.
        let can_reverse_apply = decision_chain
            .as_ref()
            .is_some_and(DecisionChainSnapshot::can_reverse_apply);
        let suggested_override_target = decision_chain
            .as_ref()
            .and_then(DecisionChainSnapshot::suggested_override_target);
        Self {
            generation,
            revision,
            status: RuleTracerStatus::Ready,
            source: None,
            targets: Vec::new(),
            failure: None,
            active_query: query,
            simulated_context: context,
            presets: Self::default_presets(),
            decision_chain,
            is_fallback_match: is_fallback,
            total_rules_evaluated,
            can_reverse_apply,
            suggested_override_target,
        }
    }

    pub fn empty(generation: u64, revision: u64) -> Self {
        Self {
            generation,
            revision,
            status: RuleTracerStatus::Empty,
            source: None,
            targets: Vec::new(),
            failure: None,
            active_query: String::new(),
            simulated_context: TrafficContextSnapshot::default(),
            presets: Self::default_presets(),
            decision_chain: None,
            is_fallback_match: false,
            total_rules_evaluated: 0,
            can_reverse_apply: false,
            suggested_override_target: None,
        }
    }

    pub fn unsupported(generation: u64, revision: u64, reason: impl Into<String>) -> Self {
        Self {
            generation,
            revision,
            status: RuleTracerStatus::Unsupported,
            failure: Some(reason.into()),
            presets: Self::default_presets(),
            ..Self::default()
        }
    }

    pub fn failed(generation: u64, revision: u64, reason: impl Into<String>) -> Self {
        Self {
            generation,
            revision,
            status: RuleTracerStatus::Failed,
            failure: Some(reason.into()),
            presets: Self::default_presets(),
            ..Self::default()
        }
    }

    pub fn unavailable(generation: u64, revision: u64, reason: impl Into<String>) -> Self {
        Self {
            generation,
            revision,
            status: RuleTracerStatus::Unknown,
            failure: Some(reason.into()),
            presets: Self::default_presets(),
            ..Self::default()
        }
    }

    pub fn is_drawable(&self) -> bool {
        self.status == RuleTracerStatus::Ready
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_fixture_has_complete_five_stage_decision_chain() {
        let snapshot = RuleTracerSnapshot::demo_fixture();
        assert!(snapshot.is_drawable());
        assert_eq!(snapshot.status, RuleTracerStatus::Ready);
        let chain = snapshot.decision_chain.expect("decision chain");
        assert_eq!(chain.nodes.len(), 5);
        assert_eq!(chain.nodes[0].stage, DecisionStageKind::Inbound);
        assert_eq!(chain.nodes[1].stage, DecisionStageKind::Sniffer);
        assert_eq!(chain.nodes[2].stage, DecisionStageKind::RuleSet);
        assert_eq!(chain.nodes[3].stage, DecisionStageKind::ProxyGroup);
        assert_eq!(chain.nodes[4].stage, DecisionStageKind::Outbound);
        assert_eq!(chain.final_outbound, "香港 IPLC 01");
        assert_eq!(chain.final_node_country.as_deref(), Some("HK"));
        assert_eq!(chain.final_node_delay_ms, Some(28));
    }

    #[test]
    fn default_presets_provide_diverse_scenarios() {
        let presets = RuleTracerSnapshot::default_presets();
        assert!(presets.len() >= 5);
        assert!(presets.iter().any(|p| p.query.contains("google.com")));
        assert!(presets.iter().any(|p| p.query.contains("1.1.1.1")));
        assert!(presets.iter().any(|p| p.query.contains("bilibili.com")));
        assert!(
            presets
                .iter()
                .all(|preset| preset.expected_target.is_none()),
            "a preset cannot predict routing without the user's profile"
        );
    }

    #[test]
    fn unsupported_tracer_is_not_drawable() {
        let snapshot = RuleTracerSnapshot::unsupported(1, 1, "offline host");
        assert!(!snapshot.is_drawable());
        assert_eq!(snapshot.status, RuleTracerStatus::Unsupported);
    }

    #[test]
    fn demo_fixture_carries_hit_audit_dead_rules_and_cidr_overlap() {
        let audit = RuleHitAuditSnapshot::demo_fixture();
        assert!(audit.total_hits > 0);
        assert!(!audit.top_hits.is_empty());
        assert!(
            audit
                .dead_rules
                .iter()
                .any(|d| d.reason == RuleDeadReason::ZeroHits)
        );
        assert!(
            audit
                .dead_rules
                .iter()
                .any(|d| d.reason == RuleDeadReason::Shadowed)
        );
        assert_eq!(audit.cidr_overlaps.len(), 1);
        assert!(audit.can_clear);
        assert!(audit.last_hit_rule.is_some());
        assert_eq!(audit.trace_count, 128);
        assert!(audit.avg_match_latency_us.is_some());
    }

    #[test]
    fn ready_snapshot_does_not_embed_an_independent_hit_audit() {
        let snapshot = RuleTracerSnapshot::ready(
            1,
            1,
            "github.com".to_owned(),
            TrafficContextSnapshot::default(),
            None,
            0,
        );
        let serialized = serde_json::to_value(&snapshot).unwrap();
        assert!(serialized.get("hit_audit").is_none());
        assert!(serialized.get("decision_chain").unwrap().is_null());
    }

    #[test]
    fn traffic_context_snapshot_defaults_new_sandbox_fields_for_legacy_json() {
        // DUAL-12-10: old payloads without the sandbox source-IP fields must
        // still deserialize; the new fields default to `None`.
        let legacy = r#"{"domain":"github.com","port":443}"#;
        let context: TrafficContextSnapshot =
            serde_json::from_str(legacy).expect("legacy context payload");
        assert_eq!(context.domain.as_deref(), Some("github.com"));
        assert_eq!(context.src_ip, None);
        assert_eq!(context.src_port, None);
        assert_eq!(context.in_port, None);
    }

    #[test]
    fn d12_08_ready_derives_reverse_apply_facts_from_the_chain() {
        let chain = DecisionChainSnapshot {
            hit_rule_index: Some(3),
            matched_rule_raw: "DOMAIN-SUFFIX,github.com,PROXY".to_owned(),
            target_proxy: "PROXY".to_owned(),
            is_fallback: false,
            ..DecisionChainSnapshot::default()
        };
        let snapshot = RuleTracerSnapshot::ready(
            1,
            1,
            "github.com".to_owned(),
            TrafficContextSnapshot::default(),
            Some(chain),
            10,
        );
        assert!(snapshot.can_reverse_apply);
        assert_eq!(
            snapshot.suggested_override_target.as_deref(),
            Some("DIRECT")
        );

        // A fallback MATCH replay cannot be reverse-applied.
        let fallback = DecisionChainSnapshot {
            hit_rule_index: Some(9),
            matched_rule_raw: "MATCH,DIRECT".to_owned(),
            target_proxy: "DIRECT".to_owned(),
            is_fallback: true,
            ..DecisionChainSnapshot::default()
        };
        let snapshot = RuleTracerSnapshot::ready(
            1,
            1,
            "example.org".to_owned(),
            TrafficContextSnapshot::default(),
            Some(fallback),
            10,
        );
        assert!(!snapshot.can_reverse_apply);
    }

    #[test]
    fn d12_08_suggested_target_flips_away_from_direct() {
        let direct = DecisionChainSnapshot {
            target_proxy: "DIRECT".to_owned(),
            ..DecisionChainSnapshot::default()
        };
        assert_eq!(
            direct.suggested_override_target().as_deref(),
            Some("REJECT")
        );
        let proxy = DecisionChainSnapshot {
            target_proxy: "GLOBAL-MEDIA".to_owned(),
            ..DecisionChainSnapshot::default()
        };
        assert_eq!(proxy.suggested_override_target().as_deref(), Some("DIRECT"));
        let blank = DecisionChainSnapshot::default();
        assert_eq!(blank.suggested_override_target(), None);
    }

    #[test]
    fn d12_08_override_result_carries_honest_failure() {
        let applied = TracerRuleOverrideResult::applied(
            2,
            "DIRECT".to_owned(),
            "DOMAIN,example.com,PROXY".to_owned(),
            "DOMAIN,example.com,DIRECT".to_owned(),
        );
        assert!(applied.is_applied());
        assert!(applied.failure_message().is_none());
        assert_eq!(
            applied.updated_rule_raw.as_deref(),
            Some("DOMAIN,example.com,DIRECT")
        );

        let rejected = TracerRuleOverrideResult::rejected(
            TracerRuleOverrideStatus::Unsupported,
            2,
            "DIRECT".to_owned(),
            "no apply capability",
        );
        assert!(!rejected.is_applied());
        assert_eq!(rejected.failure_message(), Some("no apply capability"));
    }
}
