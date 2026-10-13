//! test-intent: behavior
use super::*;
use infiltrator_contract::aggregator::{AggregationDraft, RegionalClusterSnapshot};

#[test]
fn preview_folds_observed_counters_missing_sources_groups_and_full_shared_yaml_viewport() {
    let report = AggregationReport {
        input_nodes: 8,
        total_nodes: 4,
        duplicates_removed: 2,
        renamed_nodes: 3,
        rule_renamed_nodes: 1,
        invalid_nodes_removed: 2,
        missing_sources: vec!["unreadable {total}".into()],
        groups: vec![GeneratedGroupSnapshot {
            name: "master {count}".into(),
            group_type: "select".into(),
            is_master: true,
            members: vec!["A".into(), "B".into()],
            ..Default::default()
        }],
        regions: vec![RegionalClusterSnapshot {
            iso: "HK".into(),
            group_name: "regional policy".into(),
            node_names: vec!["node".into()],
            ..Default::default()
        }],
        yaml: (1..=70).map(|line| format!("line {line}\n")).collect(),
        ..Default::default()
    };
    let summary = aggregation_counters(Some(&report), "en-US");
    assert!(summary.contains("input 8 · deduplicated 2 · normalized 3 · renamed by rules 1 · rejected by precheck 2 · output 4 nodes · regions 1"));
    assert!(summary.contains("Master selector cascades 2 members"));
    assert!(summary.contains("unreadable {total}"));
    let group = aggregation_groups(Some(&report), "en-US");
    assert!(group.contains("master {count} [Master selector]"));
    assert!(group.ends_with("2 members"));
    let preview = aggregation_yaml_preview(Some(&report), "en-US");
    assert!(preview.starts_with("Aggregation YAML (70 lines):"));
    assert!(preview.contains("\nline 60\n..."));
    assert!(!preview.contains("line 61"));
    assert!(aggregation_regions(Some(&report), "en-US").contains("regional policy (1 nodes)"));
}

#[test]
fn missing_preview_empty_groups_and_unsupported_template_store_remain_distinct() {
    let empty = AggregationReport::default();
    assert_eq!(
        aggregation_groups(None, "en-US"),
        "Policy groups: awaiting preview"
    );
    assert_eq!(
        aggregation_groups(Some(&empty), "en-US"),
        "Policy groups: none generated"
    );
    let templates = [AggregationTemplate {
        name: "{time}".into(),
        updated_at: "2026-10-05".into(),
        draft: AggregationDraft {
            target_name: "destination".into(),
            source_profiles: vec!["A".into()],
            ..Default::default()
        },
    }];
    assert!(aggregation_templates(false, &templates, "en-US").contains("unsupported"));
    assert!(!aggregation_templates(false, &templates, "en-US").contains("destination"));
    assert!(aggregation_templates(true, &templates, "en-US").contains("{time} → destination"));
    let future = GeneratedGroupSnapshot {
        name: "new strategy".into(),
        group_type: "relay".into(),
        ..Default::default()
    };
    assert!(aggregation_group_row(&future, "en-US").contains(" · relay · "));
}
