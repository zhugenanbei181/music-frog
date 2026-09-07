//! Shared read model and decision tree replay contract for the Live Rule Tracer (交互式分流追踪器).

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

/// Normalized traffic context used for simulation queries.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrafficContextSnapshot {
    pub domain: Option<String>,
    pub ip: Option<String>,
    pub port: Option<u16>,
    pub process_name: Option<String>,
    pub network: Option<String>,
    pub in_type: Option<String>,
    pub client_ip: Option<String>,
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
}

impl RuleTracerSnapshot {
    /// Standard preset chips recommended for interactive live simulation.
    pub fn default_presets() -> Vec<RuleTracerPreset> {
        vec![
            RuleTracerPreset {
                id: "google".to_owned(),
                label: "google.com".to_owned(),
                query: "google.com".to_owned(),
                expected_target: Some("PROXY".to_owned()),
                description: "国外主流搜索引擎 · 域名后缀匹配".to_owned(),
            },
            RuleTracerPreset {
                id: "github".to_owned(),
                label: "github.com".to_owned(),
                query: "github.com".to_owned(),
                expected_target: Some("PROXY".to_owned()),
                description: "开发者代码托管平台 · 域名后缀匹配".to_owned(),
            },
            RuleTracerPreset {
                id: "bilibili".to_owned(),
                label: "bilibili.com".to_owned(),
                query: "bilibili.com".to_owned(),
                expected_target: Some("DIRECT".to_owned()),
                description: "国内多媒体网站 · GeoSite / 直连分流".to_owned(),
            },
            RuleTracerPreset {
                id: "cloudflare_dns".to_owned(),
                label: "1.1.1.1:443".to_owned(),
                query: "1.1.1.1:443".to_owned(),
                expected_target: Some("DIRECT".to_owned()),
                description: "公共安全 DNS · IP-CIDR 掩码判定".to_owned(),
            },
            RuleTracerPreset {
                id: "steam".to_owned(),
                label: "steamcommunity.com".to_owned(),
                query: "steamcommunity.com".to_owned(),
                expected_target: Some("PROXY".to_owned()),
                description: "游戏社区服务 · 域名关键词分流".to_owned(),
            },
            RuleTracerPreset {
                id: "netflix".to_owned(),
                label: "netflix.com".to_owned(),
                query: "netflix.com".to_owned(),
                expected_target: Some("GLOBAL-MEDIA".to_owned()),
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
            failure: None,
            active_query: "github.com".to_owned(),
            simulated_context: TrafficContextSnapshot {
                domain: Some("github.com".to_owned()),
                ip: None,
                port: Some(443),
                process_name: Some("git.exe".to_owned()),
                network: Some("tcp".to_owned()),
                in_type: Some("mixed".to_owned()),
                client_ip: Some("127.0.0.1".to_owned()),
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
        Self {
            generation,
            revision,
            status: RuleTracerStatus::Ready,
            failure: None,
            active_query: query,
            simulated_context: context,
            presets: Self::default_presets(),
            decision_chain,
            is_fallback_match: is_fallback,
            total_rules_evaluated,
            can_reverse_apply: true,
            suggested_override_target: None,
        }
    }

    pub fn empty(generation: u64, revision: u64) -> Self {
        Self {
            generation,
            revision,
            status: RuleTracerStatus::Empty,
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
    }

    #[test]
    fn unsupported_tracer_is_not_drawable() {
        let snapshot = RuleTracerSnapshot::unsupported(1, 1, "offline host");
        assert!(!snapshot.is_drawable());
        assert_eq!(snapshot.status, RuleTracerStatus::Unsupported);
    }
}
