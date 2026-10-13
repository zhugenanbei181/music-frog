//! Shared preview text folds actual aggregation reports, preserving profile and group identities.
use infiltrator_contract::aggregator::{
    AggregationCustomGroup, AggregationReport, AggregationTemplate, GeneratedGroupSnapshot,
};
use infiltrator_shared::i18n_interpolator::localize;
use infiltrator_shared::locales::{Lang, Localizer};

pub const AGGREGATION_PREVIEW_LINES: usize = 60;
pub fn aggregation_counters(report: Option<&AggregationReport>, code: &str) -> String {
    let Some(report) = report else {
        return Lang(code).tr("aggregation_preview_pending").into_owned();
    };
    let master = report.master_group().map_or_else(String::new, |group| {
        localize(
            code,
            "aggregation_master_count",
            &[("count", group.members.len().to_string())],
        )
    });
    let missing = if report.missing_sources.is_empty() {
        String::new()
    } else {
        localize(
            code,
            "aggregation_missing_sources",
            &[("names", report.missing_sources.join(" / "))],
        )
    };
    localize(
        code,
        "aggregation_counters",
        &[
            ("input", report.input_nodes.to_string()),
            ("dedup", report.duplicates_removed.to_string()),
            ("renamed", report.renamed_nodes.to_string()),
            ("rules", report.rule_renamed_nodes.to_string()),
            ("invalid", report.invalid_nodes_removed.to_string()),
            ("total", report.total_nodes.to_string()),
            ("regions", report.regions.len().to_string()),
            ("master", master),
            ("missing", missing),
        ],
    )
}
pub fn aggregation_regions(report: Option<&AggregationReport>, code: &str) -> String {
    let Some(report) = report else {
        return Lang(code).tr("aggregation_regions_pending").into_owned();
    };
    if report.regions.is_empty() {
        return Lang(code).tr("aggregation_regions_empty").into_owned();
    }
    report
        .regions
        .iter()
        .map(|region| {
            localize(
                code,
                "aggregation_region_row",
                &[
                    ("flag", region.flag.clone()),
                    ("iso", region.iso.clone()),
                    ("label", region.label.clone()),
                    ("group", region.group_name.clone()),
                    ("count", region.node_names.len().to_string()),
                ],
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}
pub fn aggregation_group_row(group: &GeneratedGroupSnapshot, code: &str) -> String {
    let kind = match group.group_type.as_str() {
        "url-test" => Lang(code).tr("aggregator_group_urltest").into_owned(),
        "select" => Lang(code).tr("aggregator_group_select").into_owned(),
        raw => raw.to_owned(),
    };
    localize(
        code,
        "aggregation_group_row",
        &[
            ("name", group.name.clone()),
            (
                "master",
                if group.is_master {
                    Lang(code).tr("aggregation_master_tag").into_owned()
                } else {
                    String::new()
                },
            ),
            (
                "custom",
                if group.is_custom {
                    Lang(code).tr("aggregation_custom_tag").into_owned()
                } else {
                    String::new()
                },
            ),
            ("kind", kind),
            ("count", group.members.len().to_string()),
        ],
    )
}
pub fn aggregation_groups(report: Option<&AggregationReport>, code: &str) -> String {
    let Some(report) = report else {
        return Lang(code).tr("aggregation_groups_pending").into_owned();
    };
    if report.groups.is_empty() {
        return Lang(code).tr("aggregation_groups_empty").into_owned();
    }
    report
        .groups
        .iter()
        .map(|group| aggregation_group_row(group, code))
        .collect::<Vec<_>>()
        .join("\n")
}
pub fn aggregation_yaml_preview(report: Option<&AggregationReport>, code: &str) -> String {
    let Some(report) = report else {
        return Lang(code).tr("aggregation_yaml_pending").into_owned();
    };
    localize(
        code,
        "aggregation_yaml_preview",
        &[
            ("count", report.yaml.lines().count().to_string()),
            ("yaml", report.yaml_preview(AGGREGATION_PREVIEW_LINES)),
        ],
    )
}
pub fn aggregation_custom_group_row(group: &AggregationCustomGroup, code: &str) -> String {
    let keywords = if group.member_keywords.is_empty() {
        Lang(code).tr("aggregation_all_nodes").into_owned()
    } else {
        group.member_keywords.join(", ")
    };
    localize(
        code,
        "aggregation_custom_group_row",
        &[
            ("name", group.name.clone()),
            ("kind", group.group_type.clone()),
            ("keywords", keywords),
        ],
    )
}
pub fn aggregation_custom_groups(groups: &[AggregationCustomGroup], code: &str) -> String {
    if groups.is_empty() {
        return Lang(code).tr("aggregation_custom_empty").into_owned();
    }
    groups
        .iter()
        .map(|group| aggregation_custom_group_row(group, code))
        .collect::<Vec<_>>()
        .join("\n")
}
pub fn aggregation_template_row(template: &AggregationTemplate, code: &str) -> String {
    localize(
        code,
        "aggregation_template_row",
        &[
            ("name", template.name.clone()),
            ("target", template.draft.target_name.clone()),
            ("time", template.updated_at.clone()),
            ("count", template.draft.source_profiles.len().to_string()),
        ],
    )
}
pub fn aggregation_templates(
    available: bool,
    templates: &[AggregationTemplate],
    code: &str,
) -> String {
    if !available {
        return Lang(code)
            .tr("aggregation_templates_unsupported")
            .into_owned();
    }
    if templates.is_empty() {
        return Lang(code).tr("aggregation_templates_empty").into_owned();
    }
    templates
        .iter()
        .map(|template| aggregation_template_row(template, code))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
#[path = "aggregation_preview_projection_test.rs"]
mod tests;
