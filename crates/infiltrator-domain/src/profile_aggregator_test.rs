//! Headless coverage for the DUAL-08 structured aggregation plan.

use crate::filter::{ContentDedupStrategy, NodeSortOrder};
use crate::profile_aggregator::{
    AggregationPlan, MASTER_SELECT_GROUP, ProfileAggregator, REGION_GROUP_SUFFIX,
};
use crate::profile_converter::{AggregationOptions, SourceSubscription};

fn hk_jp_source() -> SourceSubscription {
    SourceSubscription::with_prefix(
        "Airport-A",
        r#"
proxies:
  - name: "🇭🇰 香港 01"
    type: ss
    server: 1.1.1.1
    port: 443
    password: pass
  - name: "🇯🇵 Tokyo 01"
    type: ss
    server: 2.2.2.2
    port: 443
    password: pass
"#,
        "AirportA",
    )
}

fn duplicate_us_source() -> SourceSubscription {
    SourceSubscription::with_prefix(
        "Airport-B",
        r#"
proxies:
  - name: "🇭🇰 香港 01"
    type: ss
    server: 1.1.1.1
    port: 443
    password: pass
  - name: "🇺🇸 US West"
    type: trojan
    server: 3.3.3.3
    port: 443
    password: pass
"#,
        "AirportB",
    )
}

fn options() -> AggregationOptions {
    AggregationOptions {
        content_dedup: ContentDedupStrategy::KeepFirst,
        normalize_country_code: true,
        remove_emojis: true,
        sort_by: NodeSortOrder::CountryCode,
        generate_proxy_groups: true,
        ..Default::default()
    }
}

fn plan() -> AggregationPlan {
    ProfileAggregator::plan(&[hk_jp_source(), duplicate_us_source()], &options()).unwrap()
}

#[test]
fn plan_reports_real_dedup_and_region_counts() {
    let plan = plan();
    // 4 input nodes; the duplicate HK server is fingerprint-identical.
    assert_eq!(plan.input_nodes, 4);
    assert_eq!(plan.total_nodes, 3);
    assert_eq!(plan.duplicates_removed, 1);
    assert_eq!(plan.source_count, 2);

    let isos: Vec<&str> = plan
        .regions
        .iter()
        .map(|region| region.iso.as_str())
        .collect();
    assert_eq!(isos, vec!["HK", "JP", "US"]);

    let hk = &plan.regions[0];
    assert_eq!(hk.label, "香港");
    assert_eq!(hk.flag, "🇭🇰");
    assert_eq!(hk.node_names.len(), 1);
    assert_eq!(hk.group_name(), format!("香港{REGION_GROUP_SUFFIX}"));
}

#[test]
fn plan_generates_master_cascade_before_concrete_nodes() {
    let plan = plan();
    let master = plan
        .groups
        .iter()
        .find(|group| group.name == MASTER_SELECT_GROUP)
        .expect("master selector group");
    assert!(master.is_master);
    assert_eq!(master.group_type, "select");

    // DUAL-08-05: the region groups cascade before the concrete node list.
    let region_names: Vec<String> = plan
        .regions
        .iter()
        .map(|region| region.group_name())
        .collect();
    let region_positions: Vec<usize> = region_names
        .iter()
        .map(|name| master.members.iter().position(|m| m == name).unwrap())
        .collect();
    let first_node_position = master
        .members
        .iter()
        .position(|member| member == &plan.regions[0].node_names[0])
        .unwrap();
    assert!(
        region_positions
            .iter()
            .all(|pos| *pos < first_node_position)
    );

    // DUAL-08-04: one url-test group per region, probing the shared endpoint.
    for region in &plan.regions {
        let group = plan
            .groups
            .iter()
            .find(|group| group.name == region.group_name())
            .expect("region url-test group");
        assert_eq!(group.group_type, "url-test");
        assert_eq!(group.members, region.node_names);
        assert!(plan.yaml.contains(&group.name));
    }
    assert!(plan.yaml.contains("url-test"));
    assert!(plan.yaml.contains("interval: 300"));
}

#[test]
fn plan_without_groups_keeps_regions_but_skips_topology() {
    let no_groups = AggregationOptions {
        generate_proxy_groups: false,
        ..options()
    };
    let plan = ProfileAggregator::plan(&[hk_jp_source()], &no_groups).unwrap();
    assert!(!plan.regions.is_empty());
    assert!(plan.groups.is_empty());
    assert!(!plan.yaml.contains("proxy-groups:"));
}

#[test]
fn plan_rejects_empty_sources() {
    let empty = SourceSubscription::new("Empty", "proxies: []\n");
    assert!(ProfileAggregator::plan(&[empty], &options()).is_err());
}

fn promo_source() -> SourceSubscription {
    SourceSubscription::with_prefix(
        "Airport-Promo",
        r#"
proxies:
  - name: "🇭🇰 香港 01-Pro"
    type: ss
    server: 1.1.1.1
    port: 443
    password: pass
  - name: "🇯🇵 Tokyo 01-Pro"
    type: ss
    server: 2.2.2.2
    port: 443
    password: pass
"#,
        "Promo",
    )
}

/// DUAL-08-08: user-authored regex rules rename nodes before cleaning and the
/// plan reports the real renamed count; invalid patterns fail typed.
#[test]
fn plan_applies_regex_rename_rules_before_clustering() {
    let renamed = AggregationOptions {
        rename_rules: vec![infiltrator_contract::aggregator::AggregationRenameRule {
            pattern: "-Pro$".to_string(),
            replacement: String::new(),
        }],
        ..options()
    };
    let plan = ProfileAggregator::plan(&[promo_source()], &renamed).unwrap();
    assert_eq!(plan.rule_renamed_nodes, 2);
    assert!(
        plan.regions
            .iter()
            .flat_map(|region| region.node_names.iter())
            .all(|name| !name.ends_with("-Pro")),
        "rename rules must run before clustering: {:?}",
        plan.regions
    );
    assert!(plan.yaml.contains("香港 01"));
    assert!(!plan.yaml.contains("-Pro"));

    let broken = AggregationOptions {
        rename_rules: vec![infiltrator_contract::aggregator::AggregationRenameRule {
            pattern: "(unclosed".to_string(),
            replacement: "x".to_string(),
        }],
        ..options()
    };
    let failure = ProfileAggregator::plan(&[promo_source()], &broken).unwrap_err();
    assert!(
        failure.to_string().contains("rename rule #1"),
        "the failing rule is named in the error: {failure}"
    );
}

/// DUAL-08-09: the precheck drops nodes missing required fields and the plan
/// publishes both the count and a bounded sample of real findings.
#[test]
fn plan_precheck_drops_invalid_nodes_and_reports_samples() {
    let mixed = SourceSubscription::new(
        "Mixed",
        r#"
proxies:
  - name: "🇭🇰 香港 01"
    type: ss
    server: 1.1.1.1
    port: 443
    cipher: aes-128-gcm
    password: pass
  - name: "Broken Port"
    type: ss
    server: 2.2.2.2
    port: 0
    cipher: aes-128-gcm
    password: pass
  - name: "Broken Key"
    type: vmess
    server: 3.3.3.3
    port: 443
"#,
    );
    let prechecked = AggregationOptions {
        availability_precheck: true,
        ..options()
    };
    let plan = ProfileAggregator::plan(std::slice::from_ref(&mixed), &prechecked).unwrap();
    assert_eq!(plan.input_nodes, 3);
    assert_eq!(plan.total_nodes, 1);
    assert_eq!(plan.invalid_nodes_removed, 2);
    assert_eq!(plan.invalid_node_samples.len(), 2);
    assert!(
        plan.invalid_node_samples.iter().any(
            |sample| sample.contains("Broken Port") && sample.contains("port must be positive")
        )
    );
    assert!(
        plan.invalid_node_samples
            .iter()
            .any(|sample| sample.contains("Broken Key") && sample.contains("uuid is required"))
    );
    assert!(!plan.yaml.contains("Broken Port"));
    assert!(!plan.yaml.contains("Broken Key"));

    // Without the precheck the same source keeps every node (opt-in cleaning).
    let unchecked = ProfileAggregator::plan(&[mixed], &options()).unwrap();
    assert_eq!(unchecked.total_nodes, 3);
    assert_eq!(unchecked.invalid_nodes_removed, 0);
    assert!(unchecked.invalid_node_samples.is_empty());
}

#[test]
fn plan_precheck_failure_names_the_dropped_nodes() {
    let all_broken = SourceSubscription::new(
        "Broken",
        r#"
proxies:
  - name: "Broken"
    type: trojan
    server: 1.1.1.1
    port: 443
"#,
    );
    let prechecked = AggregationOptions {
        availability_precheck: true,
        ..options()
    };
    let failure = ProfileAggregator::plan(&[all_broken], &prechecked).unwrap_err();
    assert!(
        failure.to_string().contains("availability precheck"),
        "the failure explains the precheck dropped every node: {failure}"
    );
}

/// DUAL-08-10: user-authored groups join the cascade with keyword-selected
/// members; colliding or unknown definitions fail typed.
#[test]
fn plan_synthesizes_custom_groups_with_keyword_members() {
    let custom = AggregationOptions {
        custom_groups: vec![
            infiltrator_contract::aggregator::AggregationCustomGroup {
                name: "流媒体专用".to_string(),
                group_type: "select".to_string(),
                member_keywords: vec!["west".to_string()],
            },
            infiltrator_contract::aggregator::AggregationCustomGroup {
                name: "游戏专用".to_string(),
                group_type: "url-test".to_string(),
                member_keywords: Vec::new(),
            },
        ],
        ..options()
    };
    let plan = ProfileAggregator::plan(&[hk_jp_source(), duplicate_us_source()], &custom).unwrap();

    let streaming = plan
        .groups
        .iter()
        .find(|group| group.name == "流媒体专用")
        .expect("custom select group");
    assert!(streaming.is_custom);
    assert_eq!(streaming.group_type, "select");
    assert_eq!(streaming.members.len(), 1);
    assert!(streaming.members[0].contains("US West"));

    let gaming = plan
        .groups
        .iter()
        .find(|group| group.name == "游戏专用")
        .expect("custom url-test group");
    assert!(gaming.is_custom);
    assert_eq!(gaming.members.len(), plan.total_nodes);

    let master = plan.groups.iter().find(|group| group.is_master);
    assert!(master.is_some());
    let members = &master.unwrap().members;
    let streaming_position = members.iter().position(|m| m == "流媒体专用").unwrap();
    let concrete = &plan.regions[0].node_names[0];
    let node_position = members
        .iter()
        .position(|m| m == concrete)
        .unwrap_or_else(|| panic!("concrete node member {concrete} missing from {members:?}"));
    assert!(
        streaming_position < node_position,
        "custom groups cascade before concrete nodes"
    );
    assert!(plan.yaml.contains("流媒体专用"));

    let colliding = AggregationOptions {
        custom_groups: vec![infiltrator_contract::aggregator::AggregationCustomGroup {
            name: MASTER_SELECT_GROUP.to_string(),
            group_type: "select".to_string(),
            member_keywords: Vec::new(),
        }],
        ..options()
    };
    let failure = ProfileAggregator::plan(&[hk_jp_source()], &colliding).unwrap_err();
    assert!(failure.to_string().contains("collides"));

    let unknown_type = AggregationOptions {
        custom_groups: vec![infiltrator_contract::aggregator::AggregationCustomGroup {
            name: "自定义".to_string(),
            group_type: "load-balance".to_string(),
            member_keywords: Vec::new(),
        }],
        ..options()
    };
    let failure = ProfileAggregator::plan(&[hk_jp_source()], &unknown_type).unwrap_err();
    assert!(failure.to_string().contains("unsupported type"));
}
