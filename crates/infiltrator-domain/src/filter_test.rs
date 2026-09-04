use super::*;
use crate::profile_converter::ProxyNodeItem;
use regex::Regex;
use serde_yaml_ng::Value;
use std::collections::HashSet;

#[test]
fn test_whitelist() {
    let rule = FilterRule {
        include_keywords: vec![Regex::new("HK").unwrap(), Regex::new("JP").unwrap()],
        ..Default::default()
    };
    let pipeline = SubscriptionFilterPipeline::new(rule);
    let names = vec!["HK-1".to_string(), "US-1".to_string(), "JP-2".to_string()];
    let (res, rep) = pipeline.filter_proxy_names(&names);
    assert_eq!(res, vec!["HK-1", "JP-2"]);
    assert_eq!(rep.passed, 2);
    assert_eq!(rep.excluded_by_whitelist, 1);
}

#[test]
fn test_blacklist() {
    let rule = FilterRule {
        exclude_keywords: vec![Regex::new("剩余流量").unwrap(), Regex::new("官网").unwrap()],
        ..Default::default()
    };
    let pipeline = SubscriptionFilterPipeline::new(rule);
    let names = vec![
        "HK-1".to_string(),
        "剩余流量: 10GB".to_string(),
        "官网".to_string(),
    ];
    let (res, rep) = pipeline.filter_proxy_names(&names);
    assert_eq!(res, vec!["HK-1"]);
    assert_eq!(rep.passed, 1);
    assert_eq!(rep.excluded_by_blacklist, 2);
}

#[test]
fn test_regex_replacement() {
    let rule = FilterRule {
        rename_rules: vec![RenameRule {
            pattern: Regex::new(r"🇭🇰 香港-(\d+)").unwrap(),
            replacement: "HK-$1".to_string(),
        }],
        ..Default::default()
    };
    let pipeline = SubscriptionFilterPipeline::new(rule);
    let names = vec!["🇭🇰 香港-01".to_string(), "🇭🇰 香港-02".to_string()];
    let (res, rep) = pipeline.filter_proxy_names(&names);
    assert_eq!(res, vec!["HK-01", "HK-02"]);
    assert_eq!(rep.passed, 2);
    assert_eq!(rep.renamed, 2);
}

#[test]
fn test_deduplication_append_index() {
    let rule = FilterRule {
        deduplication: DeduplicationStrategy::AppendIndex,
        ..Default::default()
    };
    let pipeline = SubscriptionFilterPipeline::new(rule);
    let names = vec![
        "HK-01".to_string(),
        "HK-01".to_string(),
        "HK-01".to_string(),
    ];
    let (res, rep) = pipeline.filter_proxy_names(&names);
    assert_eq!(res, vec!["HK-01", "HK-01 (1)", "HK-01 (2)"]);
    assert_eq!(rep.deduplicated, 2);
}

#[test]
fn test_yaml_transformation() {
    let yaml = r#"
proxies:
  - name: "🇭🇰 香港-01"
    type: ss
  - name: "剩余流量: 100G"
    type: ss
  - name: "JP-01"
    type: trojan
  - name: "🇭🇰 香港-01"
    type: ss
"#;
    let rule = FilterRule {
        rename_rules: vec![RenameRule {
            pattern: Regex::new(r"🇭🇰 香港-(\d+)").unwrap(),
            replacement: "HK-$1".to_string(),
        }],
        exclude_keywords: vec![Regex::new("剩余流量").unwrap()],
        exclude_types: vec!["trojan".to_string()],
        deduplication: DeduplicationStrategy::AppendIndex,
        ..Default::default()
    };
    let pipeline = SubscriptionFilterPipeline::new(rule);
    let (out, rep) = pipeline.apply_to_yaml(yaml).unwrap();

    let out_doc: Value = serde_yaml_ng::from_str(&out).unwrap();
    let proxies = out_doc.get("proxies").unwrap().as_sequence().unwrap();
    assert_eq!(proxies.len(), 2);
    assert_eq!(rep.passed, 2);
    assert_eq!(rep.excluded_by_blacklist, 1);
    assert_eq!(rep.excluded_by_type, 1);
    assert_eq!(rep.renamed, 2);
    assert_eq!(rep.deduplicated, 1);
    assert_eq!(proxies[0].get("name").unwrap().as_str().unwrap(), "HK-01");
    assert_eq!(
        proxies[1].get("name").unwrap().as_str().unwrap(),
        "HK-01 (1)"
    );
}

#[test]
fn test_filter_pipeline_regex_rename() {
    let mut pipeline = FilterPipeline::new();
    pipeline.add_stage(FilterStage::regex_rename(r"🇭🇰\s*", "HK-").unwrap());
    let mut nodes = vec![
        ProxyNodeItem::new("🇭🇰 Node 01", "ss", "1.1.1.1", 443),
        ProxyNodeItem::new("US Node 02", "ss", "1.1.1.2", 443),
    ];
    let stats = pipeline.apply_pipeline(&mut nodes);
    assert_eq!(stats.nodes_in, 2);
    assert_eq!(stats.nodes_out, 2);
    assert_eq!(stats.renamed_count, 1);
    assert_eq!(stats.dropped_count, 0);
    assert_eq!(nodes[0].name, "HK-Node 01");
    assert_eq!(nodes[1].name, "US Node 02");
}

#[test]
fn test_filter_pipeline_multiplier_override() {
    let mut pipeline = FilterPipeline::new();
    pipeline.add_stage(FilterStage::multiplier_override(r"(?i)vip", 1.5).unwrap());
    pipeline.add_stage(FilterStage::multiplier_override(r"(?i)game", 2.0).unwrap());
    let mut nodes = vec![
        ProxyNodeItem::new("VIP Node 01", "ss", "1.1.1.1", 443),
        ProxyNodeItem::new("Game Node [1.0x]", "ss", "1.1.1.2", 443),
        ProxyNodeItem::new("Standard Node", "ss", "1.1.1.3", 443),
    ];
    let stats = pipeline.apply_pipeline(&mut nodes);
    assert_eq!(stats.nodes_in, 3);
    assert_eq!(stats.nodes_out, 3);
    assert_eq!(stats.renamed_count, 2);
    assert_eq!(nodes[0].name, "VIP Node 01 [1.5x]");
    assert_eq!(nodes[1].name, "Game Node [2x]");
    assert_eq!(nodes[2].name, "Standard Node");
}

#[test]
fn test_filter_pipeline_protocol_filter() {
    let mut pipeline = FilterPipeline::new();
    pipeline.add_stage(FilterStage::protocol_filter(["ss", "trojan"]));
    let mut nodes = vec![
        ProxyNodeItem::new("SS Node", "ss", "1.1.1.1", 443),
        ProxyNodeItem::new("Trojan Node", "TROJAN", "1.1.1.2", 443),
        ProxyNodeItem::new("Vmess Node", "vmess", "1.1.1.3", 443),
        ProxyNodeItem::new("Hysteria Node", "hysteria2", "1.1.1.4", 443),
    ];
    let stats = pipeline.apply_pipeline(&mut nodes);
    assert_eq!(stats.nodes_in, 4);
    assert_eq!(stats.nodes_out, 2);
    assert_eq!(stats.dropped_count, 2);
    assert_eq!(nodes[0].name, "SS Node");
    assert_eq!(nodes[1].name, "Trojan Node");
}

#[test]
fn test_filter_pipeline_country_code_normalizer() {
    let mut pipeline = FilterPipeline::new();
    pipeline.add_stage(FilterStage::country_code_normalizer());
    let mut nodes = vec![
        ProxyNodeItem::new("🇭🇰 香港 01", "ss", "1.1.1.1", 443),
        ProxyNodeItem::new("🇯🇵 Tokyo Fast", "ss", "1.1.1.2", 443),
        ProxyNodeItem::new("[US] California", "ss", "1.1.1.3", 443),
        ProxyNodeItem::new("Custom Gateway", "ss", "1.1.1.4", 443),
    ];
    let stats = pipeline.apply_pipeline(&mut nodes);
    assert_eq!(stats.renamed_count, 2);
    assert_eq!(nodes[0].name, "[HK] 🇭🇰 香港 01");
    assert_eq!(nodes[1].name, "[JP] 🇯🇵 Tokyo Fast");
    assert_eq!(nodes[2].name, "[US] California");
    assert_eq!(nodes[3].name, "Custom Gateway");
}

#[test]
fn test_filter_pipeline_blacklist_and_whitelist() {
    let mut pipeline = FilterPipeline::new();
    pipeline.add_stage(FilterStage::keyword_blacklist(["官网", "过期"]).unwrap());
    pipeline.add_stage(FilterStage::keyword_whitelist(["HK", "US"]).unwrap());
    let mut nodes = vec![
        ProxyNodeItem::new("HK-01", "ss", "1.1.1.1", 443),
        ProxyNodeItem::new("HK 官网发布", "ss", "1.1.1.2", 443),
        ProxyNodeItem::new("JP-01", "ss", "1.1.1.3", 443),
        ProxyNodeItem::new("US-01", "ss", "1.1.1.4", 443),
    ];
    let stats = pipeline.apply_pipeline(&mut nodes);
    assert_eq!(stats.nodes_in, 4);
    assert_eq!(stats.nodes_out, 2);
    assert_eq!(stats.dropped_count, 2);
    assert_eq!(nodes[0].name, "HK-01");
    assert_eq!(nodes[1].name, "US-01");
}

#[test]
fn test_filter_pipeline_deduplicator_strategies() {
    let mut p1 = FilterPipeline::new();
    p1.add_stage(FilterStage::duplicate_deduplicator(
        DeduplicationStrategy::KeepFirst,
    ));
    let mut n1 = vec![
        ProxyNodeItem::new("A", "ss", "1.1.1.1", 1),
        ProxyNodeItem::new("A", "ss", "1.1.1.2", 2),
        ProxyNodeItem::new("B", "ss", "1.1.1.3", 3),
    ];
    let s1 = p1.apply_pipeline(&mut n1);
    assert_eq!(s1.nodes_out, 2);
    assert_eq!(s1.dropped_count, 1);
    assert_eq!(n1[0].port, 1);

    let mut p2 = FilterPipeline::new();
    p2.add_stage(FilterStage::duplicate_deduplicator(
        DeduplicationStrategy::KeepLast,
    ));
    let mut n2 = vec![
        ProxyNodeItem::new("A", "ss", "1.1.1.1", 1),
        ProxyNodeItem::new("A", "ss", "1.1.1.2", 2),
        ProxyNodeItem::new("B", "ss", "1.1.1.3", 3),
    ];
    let s2 = p2.apply_pipeline(&mut n2);
    assert_eq!(s2.nodes_out, 2);
    assert_eq!(s2.dropped_count, 1);
    assert_eq!(n2[0].port, 2);

    let mut p3 = FilterPipeline::new();
    p3.add_stage(FilterStage::duplicate_deduplicator(
        DeduplicationStrategy::AppendIndex,
    ));
    let mut n3 = vec![
        ProxyNodeItem::new("A", "ss", "1.1.1.1", 1),
        ProxyNodeItem::new("A", "ss", "1.1.1.2", 2),
        ProxyNodeItem::new("A", "ss", "1.1.1.3", 3),
    ];
    let s3 = p3.apply_pipeline(&mut n3);
    assert_eq!(s3.nodes_out, 3);
    assert_eq!(s3.renamed_count, 2);
    assert_eq!(n3[0].name, "A");
    assert_eq!(n3[1].name, "A (1)");
    assert_eq!(n3[2].name, "A (2)");
}

#[test]
fn test_composite_filter_pipeline() {
    let stages = vec![
        FilterStage::protocol_filter(["ss", "vmess"]),
        FilterStage::keyword_blacklist(["expire", "traffic"]).unwrap(),
        FilterStage::country_code_normalizer(),
        FilterStage::regex_rename(r"Node-(\d+)", "Srv-$1").unwrap(),
        FilterStage::multiplier_override(r"(?i)vip", 2.0).unwrap(),
        FilterStage::duplicate_deduplicator(DeduplicationStrategy::AppendIndex),
    ];
    let pipeline = FilterPipeline::with_stages(stages);
    assert!(!pipeline.is_empty());
    assert_eq!(pipeline.len(), 6);

    let mut nodes = vec![
        ProxyNodeItem::new("HK Node-01", "ss", "1.1.1.1", 443),
        ProxyNodeItem::new("HK Node-01", "ss", "1.1.1.2", 443),
        ProxyNodeItem::new("US VIP Node-02", "vmess", "1.1.1.3", 443),
        ProxyNodeItem::new("JP traffic remaining", "ss", "1.1.1.4", 443),
        ProxyNodeItem::new("SG Node-03", "trojan", "1.1.1.5", 443),
    ];

    let stats = pipeline.apply_pipeline(&mut nodes);
    assert_eq!(stats.nodes_in, 5);
    assert_eq!(stats.nodes_out, 3);
    assert_eq!(stats.dropped_count, 2);
    assert_eq!(nodes[0].name, "[HK] HK Srv-01");
    assert_eq!(nodes[1].name, "[HK] HK Srv-01 (1)");
    assert_eq!(nodes[2].name, "[US] US VIP Srv-02 [2x]");
}

#[test]
fn test_port_filter_and_server_filter() {
    let mut p = FilterPipeline::new();
    p.add_stage(FilterStage::port_filter(
        Some(HashSet::from([443, 8443])),
        None,
    ));
    p.add_stage(FilterStage::server_filter(true));

    let mut nodes = vec![
        ProxyNodeItem::new("Node1", "ss", "1.1.1.1", 443),
        ProxyNodeItem::new("Node2", "ss", "1.1.1.1", 80), // dropped by port
        ProxyNodeItem::new("Node3", "ss", "192.168.1.1", 443), // dropped by private IP
        ProxyNodeItem::new("Node4", "ss", "127.0.0.1", 8443), // dropped by loopback
        ProxyNodeItem::new("Node5", "ss", "example.com", 8443),
    ];

    let stats = p.apply_pipeline(&mut nodes);
    assert_eq!(stats.nodes_out, 2);
    assert_eq!(nodes[0].name, "Node1");
    assert_eq!(nodes[1].name, "Node5");
}

#[test]
fn test_node_mutator_and_strip_emojis() {
    let mut p = FilterPipeline::new();
    p.add_stage(FilterStage::remove_emojis());
    p.add_stage(FilterStage::node_mutator(NodeMutatorConfig {
        force_tls: Some(true),
        force_udp: Some(true),
        client_fingerprint: Some("chrome".to_string()),
        skip_cert_verify: Some(true),
        ..Default::default()
    }));

    let mut nodes = vec![ProxyNodeItem::new("🇭🇰 HK VIP 🚀", "ss", "1.1.1.1", 443)];

    let stats = p.apply_pipeline(&mut nodes);
    assert_eq!(stats.renamed_count, 1);
    assert_eq!(stats.mutated_count, 1);
    assert_eq!(nodes[0].name, "HK VIP");
    assert!(nodes[0].tls);
    assert_eq!(nodes[0].udp, Some(true));
    assert_eq!(nodes[0].client_fingerprint.as_deref(), Some("chrome"));
    assert_eq!(nodes[0].skip_cert_verify, Some(true));
}

#[test]
fn test_content_deduplication() {
    let mut p = FilterPipeline::new();
    p.add_stage(FilterStage::content_deduplicator(
        ContentDedupStrategy::KeepLowerMultiplier,
    ));

    let mut node1 = ProxyNodeItem::new("HK Node 1 [2x]", "ss", "1.1.1.1", 443);
    node1.password = Some("secret".into());
    let mut node2 = ProxyNodeItem::new("HK Node 2 [1x]", "ss", "1.1.1.1", 443);
    node2.password = Some("secret".into());
    let mut node3 = ProxyNodeItem::new("US Node 1", "ss", "2.2.2.2", 443);
    node3.password = Some("secret".into());

    let mut nodes = vec![node1, node2, node3];
    let stats = p.apply_pipeline(&mut nodes);
    assert_eq!(stats.nodes_out, 2);
    assert_eq!(nodes[0].name, "HK Node 2 [1x]");
    assert_eq!(nodes[1].name, "US Node 1");
}

#[test]
fn test_node_sorting() {
    let mut p = FilterPipeline::new();
    p.add_stage(FilterStage::sort_nodes(NodeSortOrder::CountryCode));

    let mut nodes = vec![
        ProxyNodeItem::new("[US] West", "ss", "1.1.1.1", 443),
        ProxyNodeItem::new("[HK] Fast", "ss", "1.1.1.2", 443),
        ProxyNodeItem::new("[JP] Tokyo", "ss", "1.1.1.3", 443),
    ];

    p.apply_pipeline(&mut nodes);
    assert_eq!(nodes[0].name, "[HK] Fast");
    assert_eq!(nodes[1].name, "[JP] Tokyo");
    assert_eq!(nodes[2].name, "[US] West");
}

#[test]
fn test_extract_country_code_and_private_ip() {
    assert_eq!(extract_country_code("🇭🇰 香港 01"), Some("HK"));
    assert_eq!(extract_country_code("[TW] 台北 02"), Some("TW"));
    assert_eq!(extract_country_code("Singapore Fast 03"), Some("SG"));
    assert_eq!(extract_country_code("Unknown Planet"), None);

    assert!(is_private_ip("127.0.0.1"));
    assert!(is_private_ip("192.168.1.100"));
    assert!(is_private_ip("10.0.0.1"));
    assert!(is_private_ip("172.20.0.1"));
    assert!(is_private_ip("localhost"));
    assert!(is_private_ip("::1"));
    assert!(!is_private_ip("8.8.8.8"));
    assert!(!is_private_ip("1.1.1.1"));
}
