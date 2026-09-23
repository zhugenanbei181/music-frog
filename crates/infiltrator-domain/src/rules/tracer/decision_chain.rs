//! Routing decision-chain snapshot builder (Inbound -> Sniffer -> RuleSet ->
//! Proxy Group -> Outbound). Split from [`super`] to honor the 800-line budget.

use crate::rules::RuleEntry;
use crate::rules::types::{ParsedRule, RuleType};
use infiltrator_contract::rule_tracer::{
    DecisionChainNode, DecisionChainSnapshot, DecisionNodeStatus, DecisionStageKind,
};

use super::{RuleTraceMatch, TrafficContext, explain_logical_ast};

/// Build a high-fidelity 5-stage routing decision chain snapshot
/// (Inbound -> Sniffer -> RuleSet / Rule -> Proxy Group -> Outbound).
#[allow(clippy::too_many_arguments)]
pub fn build_decision_chain(
    _rules: &[RuleEntry],
    context: &TrafficContext,
    matched: Option<&RuleTraceMatch>,
    parsed_rule: Option<&ParsedRule>,
    final_node: Option<&str>,
    final_node_protocol: Option<&str>,
    final_node_delay_ms: Option<u32>,
    final_node_country: Option<&str>,
    match_latency_us: u64,
) -> DecisionChainSnapshot {
    let mut nodes = Vec::new();

    // Stage 1: Inbound
    let in_type_label = context.in_type.as_deref().unwrap_or("mixed");
    let in_port_label = context.in_port.or(context.port).unwrap_or(7890);
    let net = context
        .network
        .as_deref()
        .unwrap_or("tcp")
        .to_ascii_uppercase();
    let client_ip = context
        .src_ip
        .or(context.client_ip)
        .map(|ip| ip.to_string())
        .unwrap_or_else(|| "127.0.0.1".to_string());
    nodes.push(DecisionChainNode {
        stage: DecisionStageKind::Inbound,
        title: format!("入站监听 ({})", in_type_label.to_ascii_uppercase()),
        detail: format!("{client_ip}:{in_port_label} ({net})"),
        badge: Some(format!("IN-PORT {in_port_label}")),
        status: DecisionNodeStatus::Passed,
        sub_evaluations: vec![
            format!("客户端来源 IP: {client_ip}"),
            format!("网络协议栈: {net}"),
        ],
    });

    // Stage 2: Sniffer
    let sniffed_domain = context.domain.as_deref().unwrap_or("—");
    let (sniffer_detail, sniffer_status, sniffer_badge) = if let Some(ref d) = context.domain {
        if context.port == Some(443) {
            (
                format!("TLS 协议嗅探 · 提取 SNI 域名: {d}"),
                DecisionNodeStatus::Passed,
                Some("TLS-SNI".to_string()),
            )
        } else {
            (
                format!("HTTP Host / 目标域名: {d}"),
                DecisionNodeStatus::Passed,
                Some("HTTP-HOST".to_string()),
            )
        }
    } else if let Some(ip) = context.ip {
        (
            format!("直接目标 IP 地址: {ip}"),
            DecisionNodeStatus::Bypassed,
            Some("IP-DIRECT".to_string()),
        )
    } else {
        (
            "未探测到域名，直通分流引擎".to_string(),
            DecisionNodeStatus::Bypassed,
            None,
        )
    };
    nodes.push(DecisionChainNode {
        stage: DecisionStageKind::Sniffer,
        title: "协议与域名嗅探 (Sniffer)".to_string(),
        detail: sniffer_detail,
        badge: sniffer_badge,
        status: sniffer_status,
        sub_evaluations: vec![
            format!("嗅探结果域名: {sniffed_domain}"),
            format!("目标端口: {}", context.port.unwrap_or(80)),
        ],
    });

    // Stage 3: RuleSet / Rule
    let (
        rule_title,
        rule_detail,
        rule_badge,
        rule_status,
        sub_evals,
        hit_idx,
        rule_raw,
        rule_type,
        payload,
        target,
        is_fallback,
    ) = match (matched, parsed_rule) {
        (Some(m), Some(p)) => {
            let is_match = matches!(p.rule_type, RuleType::Match);
            let badge = Some(p.rule_type.name().to_string());
            let status = if is_match {
                DecisionNodeStatus::Fallback
            } else {
                DecisionNodeStatus::Matched
            };
            let title = format!("命中规则 #{}: {}", m.index + 1, m.rule);
            let detail = format!("目标策略: {} · no-resolve={}", m.target, p.no_resolve);
            let sub_evals = match &p.rule_type {
                RuleType::Logical(l) => explain_logical_ast(&l.payload, context),
                _ => vec![format!("[PASS] 匹配命中表达式: {}", m.rule)],
            };
            (
                title,
                detail,
                badge,
                status,
                sub_evals,
                Some(m.index),
                m.rule.clone(),
                p.rule_type.name().to_string(),
                p.rule_type.payload().unwrap_or("").to_string(),
                m.target.clone(),
                is_match,
            )
        }
        _ => (
            "未匹配到显式规则，触发默认漏网之鱼".to_string(),
            "DIRECT 兜底策略".to_string(),
            Some("FALLBACK".to_string()),
            DecisionNodeStatus::Fallback,
            vec!["[FALLBACK] 遍历所有规则均未命中，直连放行".to_string()],
            None,
            "MATCH,DIRECT".to_string(),
            "MATCH".to_string(),
            String::new(),
            "DIRECT".to_string(),
            true,
        ),
    };

    nodes.push(DecisionChainNode {
        stage: DecisionStageKind::RuleSet,
        title: rule_title,
        detail: rule_detail,
        badge: rule_badge,
        status: rule_status,
        sub_evaluations: sub_evals,
    });

    // Stage 4: Proxy Group
    let group_name = if target.is_empty() {
        "DIRECT".to_string()
    } else {
        target.clone()
    };
    let is_direct_or_reject =
        group_name.eq_ignore_ascii_case("DIRECT") || group_name.eq_ignore_ascii_case("REJECT");
    let group_title = format!("策略组决策: [{group_name}]");
    let group_detail = if is_direct_or_reject {
        format!("系统保留内置策略: {group_name}")
    } else {
        format!("策略组调度: [{group_name}] -> 自动测速延迟最优")
    };
    nodes.push(DecisionChainNode {
        stage: DecisionStageKind::ProxyGroup,
        title: group_title,
        detail: group_detail,
        badge: Some(if is_direct_or_reject {
            "Builtin".to_string()
        } else {
            "Group".to_string()
        }),
        status: DecisionNodeStatus::Matched,
        sub_evaluations: vec![
            format!("调度策略目标: {group_name}"),
            format!(
                "决策模式: {}",
                if is_direct_or_reject {
                    "直连/阻断"
                } else {
                    "策略组调度"
                }
            ),
        ],
    });

    // Stage 5: Outbound. Builtin DIRECT/REJECT policies are fully known;
    // anything else without runtime exit data stays honestly unknown instead
    // of fabricating a node name, protocol, latency, or region.
    let known_outbound = final_node.is_some() || is_direct_or_reject;
    let out_node = final_node.unwrap_or(if is_direct_or_reject {
        &group_name
    } else {
        "未知出口"
    });
    let out_proto = final_node_protocol.unwrap_or(if is_direct_or_reject {
        "Direct"
    } else {
        "未知协议"
    });
    let out_delay = final_node_delay_ms;
    let out_country = final_node_country;
    let delay_str = out_delay
        .map(|d| format!(" · 延迟 {d}ms"))
        .unwrap_or_default();
    nodes.push(DecisionChainNode {
        stage: DecisionStageKind::Outbound,
        title: format!("最终出站: {out_node}"),
        detail: if known_outbound {
            format!("{out_proto}{delay_str}")
        } else {
            "暂无运行时出口数据 · 策略组调度结果未同步".to_string()
        },
        badge: out_country.map(|c| c.to_string()),
        status: if known_outbound {
            DecisionNodeStatus::Matched
        } else {
            DecisionNodeStatus::Neutral
        },
        sub_evaluations: vec![
            format!("出口节点名称: {out_node}"),
            format!("协议及加密: {out_proto}"),
            format!(
                "测速延迟: {}",
                out_delay
                    .map(|d| format!("{d}ms"))
                    .unwrap_or_else(|| "未知".to_string())
            ),
        ],
    });

    DecisionChainSnapshot {
        nodes,
        hit_rule_index: hit_idx,
        matched_rule_raw: rule_raw,
        matched_rule_type: rule_type,
        matched_payload: payload,
        target_proxy: target,
        final_outbound: out_node.to_string(),
        final_node_protocol: Some(out_proto.to_string()),
        final_node_delay_ms: out_delay,
        final_node_country: out_country.map(|c| c.to_string()),
        match_latency_us,
        is_fallback,
    }
}
