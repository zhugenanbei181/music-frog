#![allow(unused_imports)]
use super::*;

use super::*;

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
    let res1 = trace_rules(&rules, &ctx1).unwrap();
    assert_eq!(res1.index, 1);
    assert_eq!(res1.rule, "DOMAIN-SUFFIX,google.com");
    assert_eq!(res1.target, "Proxy-Group");

    let ctx2 = TrafficContext::from_query("my-youtube-video.org");
    let res2 = trace_rules(&rules, &ctx2).unwrap();
    assert_eq!(res2.index, 2);
    assert_eq!(res2.rule, "DOMAIN-KEYWORD,youtube");
    assert_eq!(res2.target, "Video-Group");

    let ctx3 = TrafficContext::from_query("unknown-site.net");
    let res3 = trace_rules(&rules, &ctx3).unwrap();
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
    let res1 = trace_rules(&rules, &ctx_ip).unwrap();
    assert_eq!(res1.index, 0);
    assert_eq!(res1.target, "LAN");

    let ctx_port = TrafficContext::new().with_port(443);
    let res2 = trace_rules(&rules, &ctx_port).unwrap();
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
    let ctx_src = TrafficContext::new().with_src_ip("10.1.2.3".parse().unwrap());
    assert_eq!(trace_rules(&rules, &ctx_src).unwrap().target, "INTERNAL");

    // 2. SRC-PORT
    let ctx_sp = TrafficContext::new().with_src_port(5500);
    assert_eq!(trace_rules(&rules, &ctx_sp).unwrap().target, "SPECIAL_SRC");

    // 3. IN-PORT
    let ctx_inport = TrafficContext::new().with_in_port(7895);
    assert_eq!(trace_rules(&rules, &ctx_inport).unwrap().target, "CLASH_IN");

    // 4. IN-TYPE
    let ctx_intype = TrafficContext::new().with_in_type("TUN");
    assert_eq!(
        trace_rules(&rules, &ctx_intype).unwrap().target,
        "TUN_PROXY"
    );

    // 5. PROCESS-PATH
    let ctx_ppath = TrafficContext::new().with_process_path("/usr/bin/curl");
    assert_eq!(
        trace_rules(&rules, &ctx_ppath).unwrap().target,
        "CURL_DIRECT"
    );

    // 6. PROCESS-NAME
    let ctx_pname = TrafficContext::new().with_process("steam.exe");
    assert_eq!(trace_rules(&rules, &ctx_pname).unwrap().target, "GAME");

    // 7. NETWORK
    let ctx_net = TrafficContext::new().with_network("udp");
    assert_eq!(trace_rules(&rules, &ctx_net).unwrap().target, "UDP_REJECT");

    // 8. DSCP
    let ctx_dscp = TrafficContext::new().with_dscp(46);
    assert_eq!(trace_rules(&rules, &ctx_dscp).unwrap().target, "VOIP_HIGH");

    // 9. UID
    let ctx_uid = TrafficContext::new().with_uid(1001);
    assert_eq!(trace_rules(&rules, &ctx_uid).unwrap().target, "USER_PROXY");

    // 10. PACKAGE-NAME
    let ctx_pkg = TrafficContext::new().with_package_name("com.google.android.youtube");
    assert_eq!(trace_rules(&rules, &ctx_pkg).unwrap().target, "YT_APP");
}

#[test]
fn test_trace_nested_logical_rules() {
    let rules = vec![
        RuleEntry {
            rule: "AND((DOMAIN-SUFFIX,openai.com),(DST-PORT,443),AI_PROXY)".into(),
            enabled: true,
        },
        RuleEntry {
            rule: "OR((DOMAIN-KEYWORD,bili),(DOMAIN-SUFFIX,qq.com),CN_DIRECT)".into(),
            enabled: true,
        },
        RuleEntry {
            rule: "NOT((NETWORK,udp),TCP_ONLY)".into(),
            enabled: true,
        },
        RuleEntry {
            rule: "SUB-RULE((DOMAIN,sub.example.com),SUB_ROUTE)".into(),
            enabled: true,
        },
        RuleEntry {
            rule: "MATCH,DIRECT".into(),
            enabled: true,
        },
    ];

    // 1. AND pass
    let ctx_and = TrafficContext::from_domain("api.openai.com").with_port(443);
    let res_and = trace_rules(&rules, &ctx_and).unwrap();
    assert_eq!(res_and.target, "AI_PROXY");

    // 2. AND fail (port 80) -> hits OR
    let ctx_or = TrafficContext::from_domain("live.bilibili.com").with_port(80);
    let res_or = trace_rules(&rules, &ctx_or).unwrap();
    assert_eq!(res_or.target, "CN_DIRECT");

    // 3. NOT pass: network is tcp -> matches NOT(NETWORK,udp)
    let ctx_not = TrafficContext::from_domain("other.org").with_network("tcp");
    let res_not = trace_rules(&rules, &ctx_not).unwrap();
    assert_eq!(res_not.target, "TCP_ONLY");

    // 4. SUB-RULE
    let ctx_sub = TrafficContext::from_domain("sub.example.com").with_network("udp");
    let res_sub = trace_rules(&rules, &ctx_sub).unwrap();
    assert_eq!(res_sub.target, "SUB_ROUTE");
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
    let matched = trace_rules(&rules, &ctx);
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
