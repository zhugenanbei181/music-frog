//! Statistics rows, pagination, action gates and read/command feedback fold once here.
use crate::rule_list_editor::RuleListEditor;
use crate::rule_statistics_projection::{RuleStatisticsProjection, project_statistics};
use crate::rule_statistics_workbench::{
    RuleStatisticsWorkbench, STATS_ROWS_PER_PAGE, StatisticsCleanupFeedback, StatisticsResetPhase,
    StatisticsTab,
};
use infiltrator_contract::rule_hit_audit::RuleDeadReason;
use infiltrator_contract::rule_source::RuleSourceIdentity;
use lurker_ui_presentation::copy::TextCatalog;
use lurker_ui_presentation::copy::interpolate;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StatisticsRowKey {
    Qualified {
        source: RuleSourceIdentity,
        tab: StatisticsTab,
        index: usize,
    },
    Unqualified {
        tab: StatisticsTab,
        position: usize,
    },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatisticsRowProjection {
    pub key: StatisticsRowKey,
    pub ordinal: String,
    pub raw: String,
    pub count: String,
    pub detail: String,
}
pub struct StatisticsInspectorProjection {
    pub metrics: RuleStatisticsProjection,
    pub status: String,
    pub source: String,
    pub last_hit: String,
    pub rows: Vec<StatisticsRowProjection>,
    pub empty_rows: String,
    pub page: String,
    pub can_previous: bool,
    pub can_next: bool,
    pub can_inspect: bool,
    pub can_prepare_cleanup: bool,
    pub can_reset: bool,
    pub feedback: String,
    pub confirmation_status: String,
    pub confirmation_summary: String,
    pub can_confirm_cleanup: bool,
}
fn copy(catalog: &dyn TextCatalog, key: &str, params: &[(&str, &str)]) -> String {
    interpolate(catalog.text(key).as_ref(), params)
}

pub fn project_inspector(
    model: &RuleStatisticsWorkbench,
    editor: &RuleListEditor,
    catalog: &dyn TextCatalog,
) -> StatisticsInspectorProjection {
    let unknown = catalog.text("shell_readout_unknown").into_owned();
    let metrics = project_statistics(model.audit.as_ref(), catalog);
    let source = model
        .source
        .as_ref()
        .map(|source| source.profile.clone())
        .unwrap_or_else(|| unknown.clone());
    let last_hit = model
        .audit
        .as_ref()
        .map(|audit| {
            audit
                .last_hit_rule
                .clone()
                .unwrap_or_else(|| catalog.text("rule_hit_none").into_owned())
        })
        .unwrap_or_else(|| unknown.clone());
    let status = if let Some(failure) = model.read_failure() {
        copy(
            catalog,
            "rules_stats_stale",
            &[("reason", &failure.message)],
        )
    } else if model.reset_request().is_some() {
        catalog.text("rules_stats_clearing").into_owned()
    } else if matches!(
        model.reset_phase(),
        StatisticsResetPhase::AwaitingReadback { .. }
    ) {
        catalog.text("rules_stats_waiting_readback").into_owned()
    } else if model.audit.is_none() {
        catalog.text("rules_trace_stats_unobserved").into_owned()
    } else {
        catalog.text("rules_stats_observed").into_owned()
    };
    let total = model.audit.as_ref().map_or(0, |audit| match model.tab {
        StatisticsTab::Summary => 0,
        StatisticsTab::TopHits => audit.top_hits.len(),
        StatisticsTab::Inactive => audit.dead_rules.len(),
    });
    let pages = total.div_ceil(STATS_ROWS_PER_PAGE).max(1);
    let page_index = model.page.min(pages - 1);
    let offset = page_index * STATS_ROWS_PER_PAGE;
    let mut rows = Vec::with_capacity(total.min(STATS_ROWS_PER_PAGE));
    if let Some(audit) = &model.audit {
        match model.tab {
            StatisticsTab::Summary => {}
            StatisticsTab::TopHits => {
                for (position, row) in audit
                    .top_hits
                    .iter()
                    .enumerate()
                    .skip(offset)
                    .take(STATS_ROWS_PER_PAGE)
                {
                    let index = row.rule_index;
                    let bytes = row
                        .total_payload_bytes
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| unknown.clone());
                    let time = row
                        .last_hit_secs
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| unknown.clone());
                    rows.push(StatisticsRowProjection {
                        key: index
                            .zip(audit.source.clone())
                            .map(|(index, source)| StatisticsRowKey::Qualified {
                                source,
                                tab: model.tab,
                                index,
                            })
                            .unwrap_or(StatisticsRowKey::Unqualified {
                                tab: model.tab,
                                position,
                            }),
                        ordinal: index
                            .map(|index| (index + 1).to_string())
                            .unwrap_or_else(|| unknown.clone()),
                        raw: row.rule_raw.clone(),
                        count: row.hit_count.to_string(),
                        detail: copy(
                            catalog,
                            "rules_stats_hit_detail",
                            &[("time", &time), ("bytes", &bytes)],
                        ),
                    });
                }
            }
            StatisticsTab::Inactive => {
                for (position, row) in audit
                    .dead_rules
                    .iter()
                    .enumerate()
                    .skip(offset)
                    .take(STATS_ROWS_PER_PAGE)
                {
                    let index = row.rule_index;
                    let reason = catalog.text(match row.reason {
                        RuleDeadReason::ZeroHits => "rules_stats_zero_local",
                        RuleDeadReason::Shadowed => "rules_stats_shadowed",
                    });
                    let mut detail = reason.into_owned();
                    if let Some(parent) = &row.shadowed_by {
                        detail.push_str(" · ");
                        detail.push_str(parent);
                    }
                    rows.push(StatisticsRowProjection {
                        key: index
                            .zip(audit.source.clone())
                            .map(|(index, source)| StatisticsRowKey::Qualified {
                                source,
                                tab: model.tab,
                                index,
                            })
                            .unwrap_or(StatisticsRowKey::Unqualified {
                                tab: model.tab,
                                position,
                            }),
                        ordinal: index
                            .map(|index| (index + 1).to_string())
                            .unwrap_or_else(|| unknown.clone()),
                        raw: row.rule_raw.clone(),
                        count: row.hit_count.to_string(),
                        detail,
                    });
                }
            }
        }
    }
    let empty_rows = if model.tab != StatisticsTab::Summary && rows.is_empty() {
        catalog
            .text(if model.audit.is_some() {
                "rules_stats_empty_rows"
            } else {
                "rules_trace_stats_unobserved"
            })
            .into_owned()
    } else {
        String::new()
    };
    let feedback = if let Some(failure) = model.reset_failure() {
        copy(
            catalog,
            "rules_stats_clear_failed",
            &[("reason", &failure.message)],
        )
    } else {
        match model.cleanup_feedback() {
            StatisticsCleanupFeedback::Completed(count) => copy(
                catalog,
                "rules_trace_disabled_rows",
                &[("count", &count.to_string())],
            ),
            StatisticsCleanupFeedback::None => {
                model.inspected_rows().map_or_else(String::new, |rows| {
                    copy(
                        catalog,
                        "rules_trace_audit_complete",
                        &[
                            ("count", &rows.len().to_string()),
                            ("total", &editor.draft.len().to_string()),
                        ],
                    )
                })
            }
        }
    };
    let can_confirm_cleanup = model.can_confirm_cleanup(editor);
    StatisticsInspectorProjection {
        metrics,
        status,
        source,
        last_hit,
        rows,
        empty_rows,
        page: copy(
            catalog,
            "rules_stats_page",
            &[
                ("page", &(page_index + 1).to_string()),
                ("pages", &pages.to_string()),
                ("total", &total.to_string()),
            ],
        ),
        can_previous: page_index > 0,
        can_next: page_index + 1 < pages,
        can_inspect: model.can_inspect(editor),
        can_prepare_cleanup: model.is_inspected()
            && !model.busy()
            && model.confirmation().is_none()
            && model.inspected_rows().is_some_and(|rows| !rows.is_empty())
            && model.can_inspect(editor),
        can_reset: model.can_reset(),
        feedback,
        confirmation_status: if model.confirmation().is_some() && !can_confirm_cleanup {
            catalog.text("rules_stats_confirmation_stale").into_owned()
        } else {
            String::new()
        },
        confirmation_summary: model
            .confirmation()
            .map_or_else(String::new, |confirmation| {
                copy(
                    catalog,
                    "rules_stats_cleanup_preview",
                    &[
                        ("profile", &confirmation.source.profile),
                        ("count", &confirmation.targets.len().to_string()),
                    ],
                )
            }),
        can_confirm_cleanup,
    }
}
