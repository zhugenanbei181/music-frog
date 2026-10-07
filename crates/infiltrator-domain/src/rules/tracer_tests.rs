use super::decision_chain::build_decision_chain;
use super::*;
use crate::rules::types::parse_rule_str;
use infiltrator_contract::rule_condition::ConditionIssue;
use infiltrator_contract::rule_trace_facts::DecisionStageFacts;
use infiltrator_contract::rule_tracer::{DecisionNodeStatus, DecisionStageKind};

#[test]
fn test_trace_domain_rules() {
    let rules = vec![
        RuleEntry {
            rule: "DOMAIN,special.com,DIRECT".into(),
            enabled: true,
        },
        RuleEntry {
            rule: "DOMAIN-SUFFIX,google.com,Proxy-Group".into(),
            enabled: true,
        },
        RuleEntry {
            rule: "DOMAIN-KEYWORD,youtube,Video-Group".into(),
            enabled: true,
        },
        RuleEntry {
            rule: "MATCH,Final-Group".into(),
            enabled: true,
        },
    ];

    let ctx1 = TrafficContext::from_query("www.google.com");
    let res1 = trace_rules(&rules, &ctx1).unwrap().unwrap();
    assert_eq!(res1.index, 1);
    assert_eq!(res1.rule, "DOMAIN-SUFFIX,google.com");
    assert_eq!(res1.target, "Proxy-Group");

    let ctx2 = TrafficContext::from_query("my-youtube-video.org");
    let res2 = trace_rules(&rules, &ctx2).unwrap().unwrap();
    assert_eq!(res2.index, 2);
    assert_eq!(res2.rule, "DOMAIN-KEYWORD,youtube");
    assert_eq!(res2.target, "Video-Group");

    let ctx3 = TrafficContext::from_query("unknown-site.net");
    let res3 = trace_rules(&rules, &ctx3).unwrap().unwrap();
    assert_eq!(res3.index, 3);
    assert_eq!(res3.rule, "MATCH");
    assert_eq!(res3.target, "Final-Group");
}

#[test]
fn test_trace_ip_cidr_and_port() {
    let rules = vec![
        RuleEntry {
            rule: "IP-CIDR,192.168.1.0/24,LAN".into(),
            enabled: true,
        },
        RuleEntry {
            rule: "DST-PORT,80/443,WEB".into(),
            enabled: true,
        },
        RuleEntry {
            rule: "MATCH,DEFAULT".into(),
            enabled: true,
        },
    ];

    let ctx_ip = TrafficContext::from_ip("192.168.1.50".parse().unwrap());
    let res1 = trace_rules(&rules, &ctx_ip).unwrap().unwrap();
    assert_eq!(res1.index, 0);
    assert_eq!(res1.target, "LAN");

    let ctx_port = TrafficContext::from_ip("192.0.2.1".parse().unwrap()).with_port(443);
    let res2 = trace_rules(&rules, &ctx_port).unwrap().unwrap();
    assert_eq!(res2.index, 1);
    assert_eq!(res2.target, "WEB");
}

#[test]
fn test_trace_28_plus_rule_matrix() {
    let rules = vec![
        RuleEntry {
            rule: "SRC-IP-CIDR,10.0.0.0/8,INTERNAL".into(),
            enabled: true,
        },
        RuleEntry {
            rule: "SRC-PORT,5000-6000,SPECIAL_SRC".into(),
            enabled: true,
        },
        RuleEntry {
            rule: "IN-PORT,7895,CLASH_IN".into(),
            enabled: true,
        },
        RuleEntry {
            rule: "IN-TYPE,TUN,TUN_PROXY".into(),
            enabled: true,
        },
        RuleEntry {
            rule: "PROCESS-PATH,/usr/bin/curl,CURL_DIRECT".into(),
            enabled: true,
        },
        RuleEntry {
            rule: "PROCESS-NAME,steam.exe,GAME".into(),
            enabled: true,
        },
        RuleEntry {
            rule: "NETWORK,udp,UDP_REJECT".into(),
            enabled: true,
        },
        RuleEntry {
            rule: "DSCP,46,VOIP_HIGH".into(),
            enabled: true,
        },
        RuleEntry {
            rule: "UID,1001,USER_PROXY".into(),
            enabled: true,
        },
        RuleEntry {
            rule: "PACKAGE-NAME,com.google.android.youtube,YT_APP".into(),
            enabled: true,
        },
        RuleEntry {
            rule: "MATCH,DIRECT".into(),
            enabled: true,
        },
    ];

    // 1. SRC-IP-CIDR
    let ctx_src = complete_context().with_src_ip("10.1.2.3".parse().unwrap());
    assert_eq!(
        trace_rules(&rules, &ctx_src).unwrap().unwrap().target,
        "INTERNAL"
    );

    // 2. SRC-PORT
    let ctx_sp = complete_context().with_src_port(5500);
    assert_eq!(
        trace_rules(&rules, &ctx_sp).unwrap().unwrap().target,
        "SPECIAL_SRC"
    );

    // 3. IN-PORT
    let ctx_inport = complete_context().with_in_port(7895);
    assert_eq!(
        trace_rules(&rules, &ctx_inport).unwrap().unwrap().target,
        "CLASH_IN"
    );

    // 4. IN-TYPE
    let ctx_intype = complete_context().with_in_type("TUN");
    assert_eq!(
        trace_rules(&rules, &ctx_intype).unwrap().unwrap().target,
        "TUN_PROXY"
    );

    // 5. PROCESS-PATH
    let ctx_ppath = complete_context().with_process_path("/usr/bin/curl");
    assert_eq!(
        trace_rules(&rules, &ctx_ppath).unwrap().unwrap().target,
        "CURL_DIRECT"
    );

    // 6. PROCESS-NAME
    let ctx_pname = complete_context().with_process("steam.exe");
    assert_eq!(
        trace_rules(&rules, &ctx_pname).unwrap().unwrap().target,
        "GAME"
    );

    // 7. NETWORK
    let ctx_net = complete_context().with_network("udp");
    assert_eq!(
        trace_rules(&rules, &ctx_net).unwrap().unwrap().target,
        "UDP_REJECT"
    );

    // 8. DSCP
    let ctx_dscp = complete_context().with_dscp(46);
    assert_eq!(
        trace_rules(&rules, &ctx_dscp).unwrap().unwrap().target,
        "VOIP_HIGH"
    );

    // 9. UID
    let ctx_uid = complete_context().with_uid(1001);
    assert_eq!(
        trace_rules(&rules, &ctx_uid).unwrap().unwrap().target,
        "USER_PROXY"
    );

    // The locked kernel rejects PACKAGE-NAME; the sandbox cannot invent support.
    let ctx_pkg = complete_context().with_package_name("com.google.android.youtube");
    let issue = trace_rules(&rules, &ctx_pkg).unwrap_err();
    assert_eq!(issue.index, 9);
    assert!(
        matches!(issue.issue, ConditionIssue::InvalidRule { reason } if reason.contains("PACKAGE-NAME"))
    );
}

#[test]
fn test_trace_nested_logical_rules() {
    let rules = vec![
        RuleEntry {
            rule: "AND,((DOMAIN-SUFFIX,openai.com),(DST-PORT,443)),AI_PROXY".into(),
            enabled: true,
        },
        RuleEntry {
            rule: "OR,((DOMAIN-KEYWORD,bili),(DOMAIN-SUFFIX,qq.com)),CN_DIRECT".into(),
            enabled: true,
        },
        RuleEntry {
            rule: "NOT,((NETWORK,udp)),TCP_ONLY".into(),
            enabled: true,
        },
        RuleEntry {
            rule: "SUB-RULE,(DOMAIN,sub.example.com),SUB_ROUTE".into(),
            enabled: true,
        },
        RuleEntry {
            rule: "MATCH,DIRECT".into(),
            enabled: true,
        },
    ];

    // 1. AND pass
    let ctx_and = TrafficContext::from_domain("api.openai.com").with_port(443);
    let res_and = trace_rules(&rules, &ctx_and).unwrap().unwrap();
    assert_eq!(res_and.target, "AI_PROXY");

    // 2. AND fail (port 80) -> hits OR
    let ctx_or = TrafficContext::from_domain("live.bilibili.com").with_port(80);
    let res_or = trace_rules(&rules, &ctx_or).unwrap().unwrap();
    assert_eq!(res_or.target, "CN_DIRECT");

    // 3. NOT pass: network is tcp -> matches NOT(NETWORK,udp)
    let ctx_not = TrafficContext::from_domain("other.org")
        .with_port(443)
        .with_network("tcp");
    let res_not = trace_rules(&rules, &ctx_not).unwrap().unwrap();
    assert_eq!(res_not.target, "TCP_ONLY");

    // 4. SUB-RULE
    let ctx_sub = TrafficContext::from_domain("sub.example.com")
        .with_port(443)
        .with_network("udp");
    let res_sub =
        trace_rules(&rules, &ctx_sub).expect_err("named sub-rule data is not an outbound");
    assert_eq!(
        res_sub.issue,
        ConditionIssue::ExternalData {
            rule_type: "SUB-RULE".into(),
            name: "SUB_ROUTE".into()
        }
    );
}

#[test]
fn test_decision_chain_generation() {
    let rules = vec![
        RuleEntry {
            rule: "DOMAIN-SUFFIX,github.com,PROXY".into(),
            enabled: true,
        },
        RuleEntry {
            rule: "MATCH,DIRECT".into(),
            enabled: true,
        },
    ];

    let ctx = TrafficContext::from_domain("github.com").with_port(443);
    let matched = trace_rules(&rules, &ctx).expect("complete sandbox");
    let parsed = matched
        .as_ref()
        .and_then(|m| parse_rule_str(&rules[m.index].rule).ok());

    let chain = build_decision_chain(
        &rules,
        &ctx,
        matched.as_ref(),
        parsed.as_ref(),
        Some("香港专线 01"),
        Some("VLESS · Reality"),
        Some(28),
        Some("HK"),
        15,
    );

    assert_eq!(chain.nodes.len(), 5);
    assert_eq!(chain.nodes[0].stage, DecisionStageKind::Inbound);
    assert_eq!(chain.nodes[1].stage, DecisionStageKind::Sniffer);
    assert_eq!(chain.nodes[2].stage, DecisionStageKind::RuleSet);
    assert_eq!(chain.nodes[3].stage, DecisionStageKind::ProxyGroup);
    assert_eq!(chain.nodes[4].stage, DecisionStageKind::Outbound);
    assert_eq!(chain.final_outbound, "香港专线 01");
    assert_eq!(chain.final_node_delay_ms, Some(28));
    assert!(!chain.is_fallback);
}

#[test]
fn test_decision_chain_without_exit_data_stays_honestly_unknown() {
    let rules = vec![RuleEntry {
        rule: "DOMAIN-SUFFIX,github.com,PROXY".into(),
        enabled: true,
    }];

    let ctx = TrafficContext::from_domain("github.com").with_port(443);
    let matched = trace_rules(&rules, &ctx).expect("complete sandbox");
    let parsed = matched
        .as_ref()
        .and_then(|m| parse_rule_str(&rules[m.index].rule).ok());

    // No runtime exit facts at all: the outbound stage must not fabricate a
    // node name, protocol, latency, or region.
    let chain = build_decision_chain(
        &rules,
        &ctx,
        matched.as_ref(),
        parsed.as_ref(),
        None,
        None,
        None,
        None,
        15,
    );

    let outbound = &chain.nodes[4];
    assert_eq!(outbound.stage, DecisionStageKind::Outbound);
    assert_eq!(outbound.status, DecisionNodeStatus::Neutral);
    assert_eq!(
        outbound.facts,
        Some(DecisionStageFacts::Outbound {
            name: None,
            protocol: None,
            delay_ms: None,
            country: None
        })
    );
    assert!(chain.final_outbound.is_empty());
    assert_eq!(chain.final_node_protocol, None);
    assert_eq!(chain.final_node_delay_ms, None);
    assert_eq!(chain.final_node_country, None);
}

#[test]
fn test_decision_chain_direct_policy_keeps_builtin_facts() {
    let rules = vec![RuleEntry {
        rule: "DOMAIN-SUFFIX,bilibili.com,DIRECT".into(),
        enabled: true,
    }];

    let ctx = TrafficContext::from_domain("bilibili.com").with_port(443);
    let matched = trace_rules(&rules, &ctx).expect("complete sandbox");
    let parsed = matched
        .as_ref()
        .and_then(|m| parse_rule_str(&rules[m.index].rule).ok());

    let chain = build_decision_chain(
        &rules,
        &ctx,
        matched.as_ref(),
        parsed.as_ref(),
        None,
        None,
        None,
        None,
        15,
    );

    assert_eq!(chain.final_outbound, "DIRECT");
    assert_eq!(chain.nodes[4].status, DecisionNodeStatus::Matched);
    assert_eq!(chain.final_node_protocol.as_deref(), Some("DIRECT"));
}

#[test]
fn decision_chain_never_invents_inbound_or_packet_sniffing_from_a_domain_query() {
    let rules = vec![RuleEntry {
        rule: "DOMAIN,example.org,PROXY".into(),
        enabled: true,
    }];
    let context = TrafficContext::from_domain("example.org").with_port(443);
    let matched = trace_rules(&rules, &context)
        .expect("complete sandbox")
        .expect("domain match");
    let parsed = parse_rule_str(&rules[0].rule).unwrap();
    let chain = build_decision_chain(
        &rules,
        &context,
        Some(&matched),
        Some(&parsed),
        None,
        None,
        None,
        None,
        1,
    );
    let Some(DecisionStageFacts::Inbound(inbound)) = &chain.nodes[0].facts else {
        panic!("inbound facts");
    };
    assert_eq!(inbound.src_ip, None);
    assert_eq!(inbound.in_port, None);
    assert_eq!(inbound.src_port, None);
    assert_eq!(inbound.network, None);
    assert_eq!(inbound.in_type, None);
    assert_eq!(chain.nodes[0].status, DecisionNodeStatus::Neutral);
    assert_eq!(chain.nodes[1].status, DecisionNodeStatus::Neutral);
    assert_eq!(
        chain.nodes[1].facts,
        Some(DecisionStageFacts::Sniffer {
            domain: Some("example.org".into()),
            ip: None,
            port: Some(443)
        })
    );
    assert_eq!(
        chain.nodes[3].facts,
        Some(DecisionStageFacts::Policy {
            target: "PROXY".into(),
            selected: None
        })
    );
    assert_eq!(chain.matched_rule_raw, rules[0].rule);
}

fn complete_context() -> TrafficContext {
    TrafficContext {
        src_ip: Some("192.0.2.1".parse().unwrap()),
        src_port: Some(1234),
        in_port: Some(1234),
        in_type: Some("mixed".into()),
        process_path: Some("/usr/bin/other".into()),
        process_name: Some("other.exe".into()),
        network: Some("tcp".into()),
        dscp: Some(0),
        uid: Some(0),
        package_name: Some("other.app".into()),
        ..TrafficContext::default()
    }
}
