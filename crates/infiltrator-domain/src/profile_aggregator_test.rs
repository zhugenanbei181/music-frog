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
