//! Shared copy keeps typed simulation facts, raw conditions and user values intact.
use super::*;
use infiltrator_contract::rule_tracer::{DecisionStageKind, RuleTracerSnapshot};
#[test]
fn projection_localizes_all_stages_and_retains_every_condition_without_fake_sniffing() {
    let chain = RuleTracerSnapshot::demo_fixture().decision_chain.unwrap();
    let en = present_chain(&chain, "en-US");
    let zh = present_chain(&chain, "zh-CN");
    assert_eq!(en.nodes.len(), 5);
    assert_eq!(zh.nodes.len(), 5);
    assert!(stage_text(&en.nodes[0], "en-US").contains("Source 127.0.0.1:58421"));
    assert!(stage_text(&en.nodes[1], "en-US").contains("No packet capture or sniffing result"));
    assert!(stage_text(&en.nodes[1], "en-US").contains("Destination port 443"));
    assert_eq!(en.nodes[1].status, DecisionNodeStatus::Neutral);
    assert!(
        stage_text(&en.nodes[2], "en-US")
            .contains("Condition · Matched · DOMAIN-SUFFIX,github.com")
    );
    assert!(stage_text(&en.nodes[3], "en-US").contains("Reported selection 香港 IPLC 01"));
    assert!(stage_text(&en.nodes[4], "en-US").contains("Delay 28 ms"));
    assert!(stage_text(&zh.nodes[1], "zh-CN").contains("没有抓包或嗅探结果"));
    assert_eq!(en.hit_rule_index, chain.hit_rule_index);
    assert_eq!(en.matched_rule_raw, chain.matched_rule_raw);
    assert_eq!(en.nodes[2].facts, chain.nodes[2].facts);
}
#[test]
fn projection_leaves_unknown_facts_neutral_and_does_not_reinterpret_user_placeholders() {
    let node = DecisionChainNode {
        stage: DecisionStageKind::ProxyGroup,
        facts: Some(DecisionStageFacts::Policy {
            target: "user {selected}".into(),
            selected: None,
        }),
        title: String::new(),
        detail: String::new(),
        badge: None,
        status: DecisionNodeStatus::Neutral,
        sub_evaluations: Vec::new(),
    };
    let copy = stage_text(&node, "en-US");
    assert!(copy.contains("Target user {selected} · Reported selection Unknown"));
    assert!(copy.contains("Unobserved / sandbox input"));
    assert!(!copy.contains("optimal"));
}
