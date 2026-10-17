//! One logical-rule expression and validation fold for native editors.
use infiltrator_contract::rule_edit::LogicalDraft;
use infiltrator_domain::rules::logical;
use lurker_ui_presentation::copy::TextCatalog;
use lurker_ui_presentation::copy::interpolate;

pub struct LogicalRuleProjection {
    pub expression: String,
    pub preview: String,
    pub selection: String,
    pub issue: String,
    pub valid: bool,
}
pub fn project_logical_rule(
    draft: &LogicalDraft,
    catalog: &dyn TextCatalog,
) -> LogicalRuleProjection {
    let expression = logical::draft_expression(draft);
    let issue = logical::draft_issue(draft);
    LogicalRuleProjection {
        preview: format!("{}: {expression}", catalog.text("subrules_result_preview")),
        expression,
        selection: interpolate(
            catalog.text("subrules_selection").as_ref(),
            &[("operator", &draft.operator)],
        ),
        valid: issue.is_none(),
        issue: issue
            .map(|issue| format!("{}: {issue}", catalog.text("subrules_validate_failed")))
            .unwrap_or_else(|| catalog.text("subrules_validate_ok").into_owned()),
    }
}
